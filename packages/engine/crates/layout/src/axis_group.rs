//! `axis-group-interface.cc` (part 3's, MS3): so far `add_element`
//! (`axis-group-interface.cc:53–74`, 22 lines), which `System::add_column`
//! calls, ported first (MS3 step 15); six of part 2's engravers call it
//! too. And its Scheme face, `ly:axis-group-interface::add-element`
//! (`axis-group-interface-scheme.cc:61–74`, 14 lines, [`PRELUDE`]), which
//! the image's `Centered_bar_number_align_engraver` calls on each
//! `CenteredBarNumber` that `Bar_number_engraver` makes, ported first
//! (MS3 step 18). An axis other than 0 or 1 is Guile's range error in
//! both, as LilyPond 2.26.0 gives it (MS3 step 21). And `get_children`
//! (`axis-group-interface.cc:628–642`, 15 lines), which part 4's
//! `consider_suicide` calls (`hara_kiri_group_spanner.rs`, MS3 step 27),
//! [`PRELUDE`]'s `%axis-group-children` since `consider_suicide` is
//! Scheme (MS3 step 28).
//!
//! The rest of the Scheme face (MS3 step 27, [`PRELUDE`]):
//! `ly:relative-group-extent` and `ly:generic-bound-extent`
//! (`axis-group-interface-scheme.cc:25–60`, 36 lines), over
//! `relative_group_extent`, `relative_maybe_bound_group_extent` and
//! `generic_bound_extent` (`axis-group-interface.cc:84–135`, 52), ported
//! first from step 28; and `internal_calc_pure_relevant_grobs`
//! (`:522–572`, 51, with `Grob_with_priority`), which
//! `ly:pure-from-neighbor-interface::calc-pure-relevant-grobs` calls
//! (`pure_from_neighbor.rs`), ported first from step 28, its sort
//! [`by_priority`]'s.
//!
//! The axis groups' extents and pure heights (MS3 step 28, [`PRELUDE`]):
//! the callbacks `ly:axis-group-interface::width`, `::height`,
//! `::pure-height`, `::calc-pure-relevant-grobs`, `::calc-pure-y-common`,
//! `::calc-x-common`, `::calc-y-common` and `::adjacent-pure-heights`
//! (`axis-group-interface.cc:138–240`, `:257–474`, `:574–626`, ≈ 290
//! lines with `relative_pure_height`, `pure_group_height`,
//! `generic_group_extent`, `sum_partial_pure_heights`,
//! `part_of_line_pure_height`, `combine_pure_heights` and `calc_common`),
//! the music crate's placeholders until then; and part 2's
//! `Item::pure_find_visible_prebroken_piece` and `break_visible`
//! (`item.cc:145–179`, 35), which they call, ported first. The break
//! ranks they read are the Paper_score's (`%break-ranks`,
//! `paper_score.rs`, MS3 step 32).
//!
//! The pure heights the breakers read for each candidate line in Rust
//! (MS4 step 1, OPEN D3): `part_of_line_pure_height` with
//! `combine_pure_heights`, `relative_pure_height` with
//! `pure_find_visible_prebroken_piece`, `pure_group_height` (the pure
//! callback `::pure-height`, which reads its columns) and
//! `adjacent_pure_heights` (`::adjacent-pure-heights`, in the callback
//! table), over `%host-call` from [`PRELUDE`] ([`HOSTS`]), whose Scheme is
//! kept for a run cancelled for the host.
//!
//! The skylines and the staff spacing (MS3 step 28, [`PRELUDE`]):
//! `ly:axis-group-interface::calc-skylines` (`skyline_spacing`, with
//! `add_grobs_of_one_priority`, `avoid_outside_staff_collisions` and
//! `outside_staff_ancestor`, `:476–483`, `:645–985`), `::combine-skylines`
//! (`:492–520`), `::calc-staff-staff-spacing` and
//! `::calc-pure-staff-staff-spacing` (`:987–1027`), the music crate's
//! placeholders until then; what they do with skyline pairs and flower's
//! `Interval_set` is Rust ([`outside_staff_move`] and the
//! `%skyline-pair…` primitives).

use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, Heap, HostHandler, Thrown, Value, Vm};
use lily_engine_skyline::SkylinePair;

use crate::R;
use crate::callbacks::Columns;
use crate::geometry::{Axis, Interval, X, Y, max};
use crate::grob;
use crate::grob_array::{self, GrobArray};
use crate::skylines::{skyline_pair_of, skyline_pair_value};
use crate::{item, spanner, unpure};

/// `add_element`: `e` an element of the axis group `me`. On each of `me`'s
/// `axes`, `me` becomes `e`'s parent if `e` has none there, and `e`'s
/// `axis-group-parent-X` or `-Y`; then `e` is added to `me`'s `elements`.
/// No axes (`axes` not a pair) is LilyPond's programming error, `e` still
/// added. Each axis is converted as it is reached (`from_scm<Axis>`), the
/// axes before it already taken.
pub fn add_element(host: &mut dyn Host, me: Value, e: Value) -> Result<(), Exception> {
    let list = grob::property(host, me, "axes")?;
    if host.heap().car(list).is_none() {
        grob::unlocated_programming_error(host, "axes should be nonempty")?;
    }
    let mut p = list;
    while let (Some(a), Some(rest)) = (host.heap().car(p), host.heap().cdr(p)) {
        p = rest;
        let heap = host.heap();
        let a = to_axis(heap, a)?;
        if grob::get_parent(heap, e, a).is_none() {
            grob::set_parent(heap, e, me, a);
        }
        let name = if a == X {
            "axis-group-parent-X"
        } else {
            "axis-group-parent-Y"
        };
        let sym = heap.intern(name);
        grob::set_object(heap, e, sym, me);
    }
    let elements = host.heap().intern("elements");
    grob_array::add_grob(host, me, elements, e)
}

/// `relative_group_extent`: the union of the extents on `a` relative to
/// `common` of `elts` that are not empty and not cross-staff, by
/// [`PRELUDE`]'s `%relative-maybe-bound-group-extent` with no bound; the
/// list of `elts` rooted until the caller unroots.
pub fn relative_group_extent(
    host: &mut dyn Host,
    elts: &[Value],
    common: Value,
    a: Axis,
) -> Result<Interval, Exception> {
    let list = host.heap().list(elts);
    host.heap().root(list);
    let axis = host.heap().integer(a as i64);
    let ext = crate::call_lily(
        host,
        "%relative-maybe-bound-group-extent",
        &[list, common, axis, Value::FALSE],
    )?;
    Ok(grob::number_pair(host.heap(), ext).unwrap_or(Interval::EMPTY))
}

/// `from_scm<Axis>` (`scm_to_unsigned_integer (a, 0, 1)`): Guile's type
/// error for no exact integer, its range error for one other than 0 or 1,
/// both from no procedure.
fn to_axis(heap: &mut Heap, a: Value) -> Result<Axis, Exception> {
    if !heap.is_exact_integer(a) {
        return Err(crate::wrong_type(heap, None, 0, a, "exact integer"));
    }
    match heap.to_i64(a) {
        Some(0) => Ok(X),
        Some(1) => Ok(Y),
        _ => {
            let key = heap.intern("out-of-range");
            let message = heap.string("Value out of range ~S to< ~S: ~S");
            let (min, max) = (heap.integer(0), heap.integer(1));
            let items = heap.list(&[min, max, a]);
            let rest = heap.list(&[a]);
            let args = heap.list(&[Value::FALSE, message, items, rest]);
            Err(Exception { key, args })
        }
    }
}

/// `(%grob-array-by-priority pairs)`: `internal_calc_pure_relevant_grobs`'
/// sort and array. The grobs of the `(grob . priority)` pairs, ordered by
/// their priorities (`Grob_with_priority`'s `operator<`) with libc++'s
/// `std::sort`, which is not stable: grobs of one priority come in the
/// order it leaves them, as in LilyPond. A new, ordered array
/// (`Grob_array::make_array`).
pub(crate) fn by_priority(cx: &mut Cx, args: &[Value]) -> crate::R {
    let heap = cx.heap();
    let mut pairs = Vec::new();
    let mut p = args[0];
    while let (Some(pair), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        let g = heap.car(pair).unwrap_or(Value::NULL);
        let priority = heap.cdr(pair).and_then(|v| heap.to_f64(v));
        pairs.push((g, priority.unwrap_or(f64::NEG_INFINITY)));
        p = rest;
    }
    lily_engine_skyline::sort(&mut pairs, &|a: &(Value, f64), b: &(Value, f64)| a.1 < b.1);
    let grobs = pairs.into_iter().map(|(g, _)| g).collect();
    Ok(cx.heap_mut().wrap(GrobArray {
        grobs,
        ordered: true,
    }))
}

// --- the pure heights in Rust (MS4 step 1) -----------------------------------

/// The interval of `v`, a pair of reals as [`PRELUDE`]'s pure heights
/// give it; else what `%scm->interval` gives, or its error.
fn interval_of(host: &mut dyn Host, v: Value) -> Result<Interval, Exception> {
    let heap = host.heap();
    if let (Some(l), Some(r)) = (heap.car(v), heap.cdr(v))
        && let (true, true) = (heap.is_real(l), heap.is_real(r))
    {
        return Ok(Interval::new(
            heap.to_f64(l).unwrap_or(0.0),
            heap.to_f64(r).unwrap_or(0.0),
        ));
    }
    let iv = crate::call_lily(host, "%scm->interval", &[v])?;
    let heap = host.heap();
    let end = |e: Option<Value>| e.and_then(|e| heap.to_f64(e)).unwrap_or(0.0);
    Ok(Interval::new(end(heap.car(iv)), end(heap.cdr(iv))))
}

fn interval_value(heap: &mut Heap, iv: Interval) -> Value {
    let (l, r) = (heap.flonum(iv.left), heap.flonum(iv.right));
    heap.cons(l, r)
}

/// `part_of_line_pure_height` ([`PRELUDE`]'s `%part-of-line-pure-height`
/// in Rust): `me`'s pure height at the beginning of a line from `start`
/// with `begin`, else in the rest of a line from `start` to `end`. Kept in
/// the spanner's pure cache as `begin-of-line-pure-height` or
/// `rest-of-line-pure-height` (`spanner.rs`); else the car or cdr of its
/// `adjacent-pure-heights` combined between `start` and `end`, `(0 . 0)`
/// where that is no pair or no vector, and for what is no spanner.
pub fn part_of_line_pure_height(
    host: &mut dyn Host,
    me: Value,
    begin: bool,
    start: usize,
    end: usize,
) -> Result<Interval, Exception> {
    let zero = Interval::new(0.0, 0.0);
    let heap = host.heap();
    if !spanner::is_spanner(heap, me) {
        return Ok(zero);
    }
    let name = if begin {
        "begin-of-line-pure-height"
    } else {
        "rest-of-line-pure-height"
    };
    let sym = heap.intern(name);
    // A line a held line of marks had read nested (B28, D272).
    let fresh = grob::with_grob_mut(heap, me, |g| core::mem::take(&mut g.pure_fresh)) == Some(true);
    if !fresh
        && let Some(cached) = spanner::get_cached_pure_property(heap, me, sym, start, end)
        && heap.car(cached).is_some()
    {
        return Ok(grob::number_pair(heap, cached).unwrap_or(zero));
    }
    let heights = grob::property(host, me, "adjacent-pure-heights")?;
    let heap = host.heap();
    let these = if begin {
        heap.car(heights)
    } else {
        heap.cdr(heights)
    };
    let ret = match these {
        Some(v) if heap.vector_items(v).is_some() => combine_pure_heights(host, me, v, start, end)?,
        _ => zero,
    };
    let heap = host.heap();
    let value = interval_value(heap, ret);
    spanner::cache_pure_property(heap, me, sym, start, end, value);
    Ok(ret)
}

/// `(%break-ranks me)`, as [`PRELUDE`] reads it: the break ranks of `me`'s
/// root system (`paper_score.rs`), its fixnums in order.
fn break_ranks(host: &mut dyn Host, me: Value) -> Result<Vec<u64>, Exception> {
    let v = crate::call_lily(host, "%break-ranks", &[me])?;
    let heap = host.heap();
    let ranks = heap.vector_items(v).unwrap_or(&[]);
    Ok(ranks
        .iter()
        .map(|r| r.as_fixnum().unwrap_or(0) as u64)
        .collect())
}

/// `combine_pure_heights`: the union of the vector `extents`' entries from
/// the break at or after `start` to the last break before `end`, by the
/// break ranks of `me`'s root system ([`break_ranks`]).
fn combine_pure_heights(
    host: &mut dyn Host,
    me: Value,
    extents: Value,
    start: usize,
    end: usize,
) -> Result<Interval, Exception> {
    let ranks = break_ranks(host, me)?;
    let (start, end) = (start as u64, end as u64);
    let mut ext = Interval::EMPTY;
    let mut i = ranks.partition_point(|&r| r < start);
    while i + 1 < ranks.len() && ranks[i] < end {
        let entry = host
            .heap()
            .vector_items(extents)
            .and_then(|e| e.get(i).copied());
        let entry = match entry {
            Some(entry) => entry,
            None => {
                let k = host.heap().integer(i as i64);
                crate::call_lily(host, "vector-ref", &[extents, k])?
            }
        };
        ext.unite(interval_of(host, entry)?);
        i += 1;
    }
    Ok(ext)
}

/// `relative_pure_height` ([`PRELUDE`]'s `%relative-pure-height` in
/// Rust): an alignment's child's is the union of its heights at the
/// beginning of a line and in the rest of one (`sum_partial_pure_heights`);
/// another group's, the union of the pure heights relative to its
/// `pure-Y-common` of its `pure-relevant-grobs` visible between `start`
/// and `end` and spanning a column there, but cross-staff stems.
fn relative_pure_height(
    host: &mut dyn Host,
    me: Value,
    start: usize,
    end: usize,
) -> Result<Interval, Exception> {
    let heap = host.heap();
    let parent = grob::get_parent(heap, me, Y);
    if parent.is_some_and(|p| grob::has_interface(heap, p, "align-interface")) {
        let mut iv = part_of_line_pure_height(host, me, true, start, start + 1)?;
        iv.unite(part_of_line_pure_height(host, me, false, start, end)?);
        return Ok(iv);
    }
    let sym = host.heap().intern("pure-Y-common");
    let common = grob::object(host, me, sym)?;
    let common = match grob::with_grob(host.heap(), common, |_| ()) {
        Some(()) => common,
        None => Value::NULL,
    };
    let mut r = Interval::EMPTY;
    for e in grob_array::extract_grob_array(host, me, "pure-relevant-grobs")? {
        let Some(g) = pure_visible_piece(host, e, start as u64, end as u64)? else {
            continue;
        };
        let (left, right) = item::spanned_column_rank_interval(host.heap(), g);
        if left > end as u64 || right < start as u64 {
            continue;
        }
        if grob::property(host, g, "cross-staff")? == Value::TRUE
            && grob::has_interface(host.heap(), g, "stem-interface")
        {
            continue;
        }
        let dims = unpure::pure_y_extent(host, g, common, start, end)?;
        if !dims.is_empty() {
            r.unite(dims);
        }
    }
    Ok(r)
}

/// `pure_find_visible_prebroken_piece` ([`PRELUDE`]'s
/// `%grob-pure-visible-piece` in Rust): a spanner itself; an item on the
/// column of rank `start` its piece at the beginning of a line, of rank
/// `end` its piece at the end of one, else itself, if that is visible
/// there (`Item::break_visible`); none for an item without a column.
fn pure_visible_piece(
    host: &mut dyn Host,
    g: Value,
    start: u64,
    end: u64,
) -> Result<Option<Value>, Exception> {
    let heap = host.heap();
    if !item::is_item(heap, g) {
        return Ok(Some(g));
    }
    let Some(col) = item::get_column(heap, g) else {
        return Ok(None);
    };
    let (rank, _) = item::spanned_column_rank_interval(heap, col);
    let it = if rank == start {
        item::find_prebroken_piece(heap, g, 1)
    } else if rank == end {
        item::find_prebroken_piece(heap, g, -1)
    } else {
        Some(g)
    };
    let Some(it) = it else {
        return Ok(None);
    };
    let vis = grob::property(host, it, "break-visibility")?;
    let heap = host.heap();
    let Some(items) = heap.vector_items(vis) else {
        return Ok(Some(it));
    };
    let i = (item::break_status_dir(heap, it) + 1) as usize;
    let entry = match items.get(i) {
        Some(&entry) => entry,
        None => {
            let k = heap.integer(i as i64);
            crate::call_lily(host, "vector-ref", &[vis, k])?
        }
    };
    Ok((entry == Value::TRUE).then_some(it))
}

/// `pure_group_height`, `ly:axis-group-interface::pure-height` in Rust
/// ([`PRELUDE`]'s `%pure-group-height`): the relative pure height less
/// `me`'s pure coordinate on its `pure-Y-common`, read first; LilyPond's
/// programming error, and the empty interval, for no common refpoint.
fn pure_height(
    host: &mut dyn Host,
    me: Value,
    start: usize,
    end: usize,
) -> Result<Value, Exception> {
    let sym = host.heap().intern("pure-Y-common");
    let common = grob::object(host, me, sym)?;
    if grob::with_grob(host.heap(), common, |_| ()).is_none() {
        grob::unlocated_programming_error(host, "no pure Y common refpoint")?;
        return Ok(interval_value(host.heap(), Interval::EMPTY));
    }
    let my_coord = unpure::pure_relative_y_coordinate(host, me, common, start, end)?;
    let r = relative_pure_height(host, me, start, end)?;
    let iv = Interval::new(r.left - my_coord, r.right - my_coord);
    Ok(interval_value(host.heap(), iv))
}

/// `adjacent_pure_heights`, `ly:axis-group-interface::adjacent-pure-heights`
/// in Rust ([`PRELUDE`]'s `%adjacent-pure-heights`): for each measure
/// between two break ranks, the union of the pure heights of `me`'s
/// `pure-relevant-grobs` at the beginning of a line and in the rest of
/// one, as a pair of vectors. An outside-staff grob is put beyond the
/// in-staff heights found before the first such grob (`union_disjoint` by
/// its `outside-staff-padding`, 0.46 by default, and its `direction`'s
/// data, up for none or 0). Cross-staff grobs are left out, as dead ones
/// without a column.
fn adjacent_pure_heights(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let mut heights = Heights::begin(host, me)?;
    while !heights.done() {
        heights.next(host)?;
    }
    Ok(heights.end(host))
}

/// [`adjacent_pure_heights`] under way, a grob at a time (B23, D261): a line's
/// heights are one read, the first that wants them pays for them all, and
/// this is what lets `system.rs` read them in steps of its own.
pub struct Heights {
    common: Value,
    elts: Vec<Value>,
    /// The next of `elts`.
    i: usize,
    ranks: Vec<u64>,
    begin_heights: Vec<Interval>,
    mid_heights: Vec<Interval>,
    staff_heights: Option<(Vec<Interval>, Vec<Interval>)>,
    direction: Value,
    /// The grob under way and its next measure.
    at: Option<Measure>,
    /// Whether it is to be held before its first outside-staff measure
    /// (B28, D272, `system.rs`).
    pub hold: bool,
}

/// A grob of [`Heights`] and its next measure.
#[derive(Clone, Copy)]
struct Measure {
    g: Value,
    j: usize,
    span: (u64, u64),
    outside_staff: bool,
    padding: f64,
    d: f64,
}

impl Heights {
    /// The read of `me`'s heights begun.
    pub fn begin(host: &mut dyn Host, me: Value) -> Result<Heights, Exception> {
        let sym = host.heap().intern("pure-Y-common");
        let common = grob::object(host, me, sym)?;
        let common = match grob::with_grob(host.heap(), common, |_| ()) {
            Some(()) => common,
            None => Value::NULL,
        };
        let elts = grob_array::extract_grob_array(host, me, "pure-relevant-grobs")?;
        let ranks = break_ranks(host, me)?;
        let n = ranks.len();
        let begin_heights = alloc::vec![Interval::EMPTY; n.saturating_sub(1)];
        let mid_heights = begin_heights.clone();
        Ok(Heights {
            common,
            elts,
            i: 0,
            ranks,
            begin_heights,
            mid_heights,
            staff_heights: None,
            direction: host.heap().intern("direction"),
            at: None,
            hold: false,
        })
    }

    /// Whether every grob has been seen.
    pub fn done(&self) -> bool {
        self.at.is_none() && self.i >= self.elts.len()
    }

    /// The next grob, or the next measure of the grob under way: a spanner
    /// the width of the score has its part of each.
    pub fn next(&mut self, host: &mut dyn Host) -> Result<(), Exception> {
        match self.at.take() {
            Some(at) => self.measure(host, at),
            None => self.grob(host),
        }
    }

    /// The next grob, to its first measure.
    fn grob(&mut self, host: &mut dyn Host) -> Result<(), Exception> {
        let g = self.elts[self.i];
        self.i += 1;
        if grob::property(host, g, "cross-staff")? == Value::TRUE {
            return Ok(());
        }
        let heap = host.heap();
        if !grob::is_live_grob(heap, g)
            && !(item::is_item(heap, g) && item::get_column(heap, g).is_some())
        {
            return Ok(());
        }
        let priority = grob::property(host, g, "outside-staff-priority")?;
        let outside_staff = host.heap().is_number(priority);
        let padding = grob::property(host, g, "outside-staff-padding")?;
        let heap = host.heap();
        let padding = if heap.is_real(padding) {
            heap.to_f64(padding).unwrap_or(0.0)
        } else {
            0.46
        };
        let data = match lily_engine_music::grob_of(heap, g) {
            Some((arena, id)) => grob::property_data(heap, &arena, id, g, self.direction),
            None => Value::NULL,
        };
        let d = if data.as_fixnum() == Some(-1) {
            -1.0
        } else {
            1.0
        };
        let span = item::spanned_column_rank_interval(host.heap(), g);
        let first_break = self
            .ranks
            .partition_point(|&r| r < span.0)
            .saturating_sub(1);
        if outside_staff && self.staff_heights.is_none() {
            self.staff_heights = Some((self.begin_heights.clone(), self.mid_heights.clone()));
        }
        let at = Measure {
            g,
            j: first_break,
            span,
            outside_staff,
            padding,
            d,
        };
        if self.hold && outside_staff {
            self.at = Some(at);
            return Ok(());
        }
        self.measure(host, at)
    }

    /// The columns of the measure it is held before (B28, D272), if it is.
    pub fn holding(&self) -> Option<(u64, u64)> {
        let at = self.at.filter(|m| self.hold && m.outside_staff)?;
        let (start, end) = (*self.ranks.get(at.j)?, *self.ranks.get(at.j + 1)?);
        (start <= at.span.1).then_some((start, end))
    }

    /// `at`'s measure, and the one after it next if the grob reaches it.
    fn measure(&mut self, host: &mut dyn Host, at: Measure) -> Result<(), Exception> {
        let Measure {
            g,
            j,
            span: (span_left, span_right),
            outside_staff,
            padding,
            d,
        } = at;
        let n = self.ranks.len();
        if !(j + 1 < n && self.ranks[j] <= span_right) {
            return Ok(());
        }
        let (start, end) = (self.ranks[j], self.ranks[j + 1]);
        let visibility_end = if j + 2 < n { self.ranks[j + 2] } else { end };
        crate::queue::check(crate::queue::Step::Box);
        if let Some(subst) = pure_visible_piece(host, g, start, visibility_end)? {
            let dims =
                unpure::pure_y_extent(host, subst, self.common, start as usize, end as usize)?;
            if !dims.is_empty() {
                let disjoint = |iv: Interval| union_disjoint(iv, dims, padding, d);
                if span_left <= start {
                    let h = match (&self.staff_heights, outside_staff) {
                        (Some((begin, _)), true) => disjoint(begin[j]),
                        _ => dims,
                    };
                    self.begin_heights[j].unite(h);
                }
                if span_right > start {
                    let h = match (&self.staff_heights, outside_staff) {
                        (Some((_, mid)), true) => disjoint(mid[j]),
                        _ => dims,
                    };
                    self.mid_heights[j].unite(h);
                }
            }
        }
        self.at = Some(Measure { j: j + 1, ..at });
        Ok(())
    }

    /// The heights, as a pair of vectors.
    pub fn end(self, host: &mut dyn Host) -> Value {
        let heap = host.heap();
        let mut vector = |ivs: Vec<Interval>| {
            let items = ivs.into_iter().map(|iv| interval_value(heap, iv)).collect();
            heap.vector(items)
        };
        let (begin, mid) = (vector(self.begin_heights), vector(self.mid_heights));
        heap.cons(begin, mid)
    }
}

/// `union_disjoint`: `iv` united with `h` moved in the direction `d` (1 or
/// -1) past `iv` by `padding`, if that moves it away from `iv`.
fn union_disjoint(mut iv: Interval, h: Interval, padding: f64, d: f64) -> Interval {
    let (edge, facing) = if d == 1.0 {
        (iv.right, h.left)
    } else {
        (iv.left, h.right)
    };
    let translation = d * ((edge + d * padding) - facing);
    iv.unite(if translation > 0.0 {
        Interval::new(h.left + translation, h.right + translation)
    } else {
        h
    });
    iv
}

/// The axis groups' callbacks in Rust, which [`HOSTS`] runs by name and
/// the callback table calls (`callbacks.rs`).
pub(crate) const CALLBACKS: [(&str, crate::callbacks::Callback); 1] =
    [("adjacent-pure-heights", adjacent_pure_heights)];

/// The pure callbacks in Rust that read their columns, which the callback
/// table calls (`callbacks.rs`).
pub(crate) const PURE_CALLBACKS: [(&str, Columns); 1] = [("pure-height", pure_height)];

/// What `%host-call` runs for [`PRELUDE`]'s `(%host-call 'axis-group what
/// me …)` (D127): `::pure-height` and `%part-of-line-pure-height` in Rust,
/// with the VM as their host; through their Scheme for columns that are no
/// `usize`.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "axis-group",
    f: host_axis_group,
}];

fn host_axis_group(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    if vm.heap().name(args[1]) == Some("adjacent-pure-heights") {
        return adjacent_pure_heights(vm, args[2]);
    }
    let heap = vm.heap();
    let column = |v: Value| {
        let n = heap.is_exact_integer(v).then(|| heap.to_i64(v)).flatten();
        n.and_then(|n| usize::try_from(n).ok())
    };
    if heap.name(args[1]) == Some("pure-height") {
        let columns = (column(args[3]), column(args[4]));
        let (Some(start), Some(end)) = columns else {
            return crate::call_lily(vm, "%pure-group-height", &args[2..5]);
        };
        return pure_height(vm, args[2], start, end);
    }
    let columns = (column(args[4]), column(args[5]));
    let (Some(start), Some(end)) = columns else {
        return crate::call_lily(vm, "%scheme-part-of-line-pure-height", &args[2..6]);
    };
    let iv = part_of_line_pure_height(vm, args[2], args[3] != Value::FALSE, start, end)?;
    Ok(interval_value(vm.heap_mut(), iv))
}

// --- the skylines (axis-group-interface.cc:476–520, :645–985) ---------------

/// `from_scm<Skyline_pair>` of `v`, the argument `pos` of `subr`: its
/// skylines, or the type error for what is no pair of two skylines.
fn skyline_pair_arg(cx: &mut Cx, subr: &str, pos: usize, v: Value) -> Result<SkylinePair, Thrown> {
    match skyline_pair_of(cx, v)? {
        Some(pair) => Ok(pair),
        None => Err(cx.wrong_type(subr, pos, v, "Skyline_pair")),
    }
}

/// The reals of the list `l`, 0 for what is none.
fn reals(heap: &Heap, mut l: Value) -> Vec<f64> {
    let mut out = Vec::new();
    while let (Some(v), Some(rest)) = (heap.car(l), heap.cdr(l)) {
        out.push(heap.to_f64(v).unwrap_or(0.0));
        l = rest;
    }
    out
}

/// The skyline pairs of the list `l`, the argument `pos` of `subr`.
fn skyline_pairs(
    cx: &mut Cx,
    subr: &str,
    pos: usize,
    mut l: Value,
) -> Result<Vec<SkylinePair>, Thrown> {
    let mut out = Vec::new();
    while let (Some(v), Some(rest)) = (cx.heap().car(l), cx.heap().cdr(l)) {
        out.push(skyline_pair_arg(cx, subr, pos, v)?);
        l = rest;
    }
    Ok(out)
}

/// `(%skyline-pair-empty? skyp)`: `Skyline_pair::is_empty`.
pub(crate) fn skyline_pair_is_empty(cx: &mut Cx, args: &[Value]) -> R {
    let pair = skyline_pair_arg(cx, "%skyline-pair-empty?", 1, args[0])?;
    Ok(Value::bool(pair.is_empty()))
}

/// `(%skyline-pair-shifted skyp s)`: a copy of `skyp` moved by `s` along
/// its horizon (`Skyline_pair::shift`).
pub(crate) fn skyline_pair_shifted(cx: &mut Cx, args: &[Value]) -> R {
    let mut pair = skyline_pair_arg(cx, "%skyline-pair-shifted", 1, args[0])?;
    pair.shift(cx.heap().to_f64(args[1]).unwrap_or(0.0));
    Ok(skyline_pair_value(cx.heap_mut(), pair))
}

/// `(%skyline-pair-raised skyp r)`: a copy of `skyp` raised by `r`
/// (`Skyline_pair::raise`).
pub(crate) fn skyline_pair_raised(cx: &mut Cx, args: &[Value]) -> R {
    let mut pair = skyline_pair_arg(cx, "%skyline-pair-raised", 1, args[0])?;
    pair.raise(cx.heap().to_f64(args[1]).unwrap_or(0.0));
    Ok(skyline_pair_value(cx.heap_mut(), pair))
}

/// `(%skyline-pairs-union pairs)`: `Skyline_pair (std::vector<Skyline_pair>)`
/// of the list `pairs`, merged pairwise; no buildings for none.
pub(crate) fn skyline_pairs_union(cx: &mut Cx, args: &[Value]) -> R {
    let pairs = skyline_pairs(cx, "%skyline-pairs-union", 1, args[0])?;
    let pair = SkylinePair::from_pairs(&pairs);
    Ok(skyline_pair_value(cx.heap_mut(), pair))
}

/// `(%skyline-pairs-merged base pairs)`: a copy of `base`, or
/// `Skyline_pair ()` for `#f`, each of the list `pairs` merged into it in
/// turn (`Skyline_pair::merge`).
pub(crate) fn skyline_pairs_merged(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "%skyline-pairs-merged";
    let mut ret = if args[0] == Value::FALSE {
        SkylinePair::default()
    } else {
        skyline_pair_arg(cx, SUBR, 1, args[0])?
    };
    for pair in skyline_pairs(cx, SUBR, 2, args[1])? {
        ret.merge(&pair);
    }
    Ok(skyline_pair_value(cx.heap_mut(), ret))
}

/// `Interval_set::interval_union`: the intervals `ivs`, sorted by their
/// left ends with libc++'s `std::sort` (`Interval::left_less`), each
/// merged into the last that it meets; one that does not, and is empty,
/// is left out, but for the first.
pub(crate) fn interval_union(mut ivs: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    lily_engine_skyline::sort(&mut ivs, &|a: &(f64, f64), b: &(f64, f64)| a.0 < b.0);
    let mut ret: Vec<(f64, f64)> = Vec::new();
    for iv in ivs {
        match ret.last_mut() {
            None => ret.push(iv),
            Some(last) if last.1 >= iv.0 => last.1 = max(last.1, iv.1),
            // `Interval::is_empty`, left above right.
            Some(_) if iv.0 > iv.1 => {}
            Some(_) => ret.push(iv),
        }
    }
    ret
}

/// `Interval_set::complement`: the gaps between `set`'s intervals and
/// beyond them, the whole line for none.
pub(crate) fn complement(set: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let (Some(first), Some(last)) = (set.first(), set.last()) else {
        return Vec::from([(f64::NEG_INFINITY, f64::INFINITY)]);
    };
    let mut ret = Vec::new();
    if first.0 > f64::NEG_INFINITY {
        ret.push((f64::NEG_INFINITY, first.0));
    }
    for w in set.windows(2) {
        ret.push((w[0].1, w[1].0));
    }
    if last.1 < f64::INFINITY {
        ret.push((last.1, f64::INFINITY));
    }
    ret
}

/// `Interval_set::nearest_point`: `x` if an interval of `set` holds it,
/// else the nearest end of one in the direction `d` (`RIGHT` 1 or `LEFT`
/// -1), or the nearer of both for 0; an infinity where there is none.
// `!(x < left)` is `std::upper_bound`'s test, NaN as it takes it.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn nearest_point(set: &[(f64, f64)], x: f64, d: i64) -> f64 {
    let mut left = f64::NEG_INFINITY;
    let mut right = f64::INFINITY;
    // upper_bound: the first interval whose left end is above x.
    let i = set.partition_point(|iv| !(x < iv.0));
    if let Some(iv) = set.get(i) {
        right = iv.0;
    }
    if i > 0 {
        let left_iv = set[i - 1];
        if left_iv.1 >= x {
            return x;
        }
        left = left_iv.1;
    }
    match d {
        1 => right,
        -1 => left,
        _ if (right - x) < (x - left) => right,
        _ => left,
    }
}

/// `(%outside-staff-move skyp padding horizon-padding others paddings
/// horizon-paddings dir)`: how far `avoid_outside_staff_collisions` moves
/// the outside-staff grob of the skylines `skyp` in the direction `dir`
/// to clear each of `others`, with the larger of its paddings and each
/// one's: the point of the shifts allowed nearest 0 that way. A padding
/// along the horizon below 0 is `Skyline::padded`'s warning, once a
/// distance.
pub(crate) fn outside_staff_move(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "%outside-staff-move";
    let v = skyline_pair_arg(cx, SUBR, 1, args[0])?;
    let heap = cx.heap();
    let padding = heap.to_f64(args[1]).unwrap_or(0.0);
    let horizon_padding = heap.to_f64(args[2]).unwrap_or(0.0);
    let (other_padding, other_horizon_padding) = (reals(heap, args[4]), reals(heap, args[5]));
    let dir = heap.to_i64(args[6]).unwrap_or(1);
    let others = skyline_pairs(cx, SUBR, 4, args[3])?;
    let mut warnings = Vec::new();
    let mut forbidden = Vec::with_capacity(others.len());
    // `Skyline::distance` pads `v` again for each of `others`; it is padded
    // once for each horizon padding instead, which measures the same.
    let mut pads: Vec<(f64, SkylinePair)> = Vec::new();
    for (j, other) in others.iter().enumerate() {
        let pad = max(padding, other_padding.get(j).copied().unwrap_or(0.0));
        let horizon_pad = max(
            horizon_padding,
            other_horizon_padding.get(j).copied().unwrap_or(0.0),
        );
        for _ in 0..2 {
            if horizon_pad < 0.0 {
                warnings.push(String::from(
                    "Cannot have negative horizon padding.  Junking.",
                ));
            }
        }
        let padded = if horizon_pad <= 0.0 {
            &v
        } else {
            let k = match pads
                .iter()
                .position(|(h, _)| h.to_bits() == horizon_pad.to_bits())
            {
                Some(k) => k,
                None => {
                    let p = SkylinePair::new(v.down.padded(horizon_pad), v.up.padded(horizon_pad));
                    pads.push((horizon_pad, p));
                    pads.len() - 1
                }
            };
            &pads[k].1
        };
        // How far up to be above `other`, and down to be below it.
        let up = padded.down.distance(&other.up, 0.0) + pad;
        let down = padded.up.distance(&other.down, 0.0) + pad;
        forbidden.push((-down, up));
    }
    let allowed = complement(&interval_union(forbidden));
    let shift = nearest_point(&allowed, 0.0, dir);
    let shift = cx.heap_mut().flonum(shift);
    crate::warned(cx, shift, &warnings)
}

/// `ly:axis-group-interface::add-element`: [`add_element`] in Scheme, as a
/// primitive has no [`Host`] to read `axes` through. And the rest, Scheme
/// for the same reason:
///
/// - `%interval-unite`, `Interval::unite` (`std::min` and `std::max` of
///   each end, as `geometry.rs`' are).
/// - `%relative-maybe-bound-group-extent elts common a bound`: the union of
///   the extents of `elts` relative to `common` that are not empty, of
///   each grob whose `cross-staff` is not `#t`; with `bound`, an axis
///   group's is its `%generic-bound-extent` instead.
/// - `%generic-bound-extent me common a`: after `vertical-skylines` is read
///   on Y, the extent of `me`'s elements with one of its
///   `bound-alignment-interfaces`, an element once for each (as LilyPond
///   collects them), relative to `common`, or to their common refpoint
///   with `me` for `common` `'()` (no grob); `me`'s robust extent when
///   there are none.
/// - `%pure-relevant-grobs me sym`: `internal_calc_pure_relevant_grobs`,
///   the grobs of `me`'s `sym` but the broken pieces of items, each with
///   its `outside-staff-priority` (a real, else -infinity), ordered by it.
/// - `ly:relative-group-extent` and `ly:generic-bound-extent`, whose
///   argument errors are LilyPond's, the first one's from
///   `ly_relative_group_extent` (`SCM_ASSERT_TYPE`'s `__FUNCTION__`). A
///   list's grobs go through `grob_list_to_grob_array`, with its warning
///   for what is no grob.
///
/// And the axis groups' extents and pure heights (MS3 step 28), their type
/// errors LilyPond's, named by `MAKE_SCHEME_CALLBACK`'s C++ method
/// (`ly:width`, `ly:pure-height`, …, LilyPond 2.26.0's):
///
/// - `%group-common me elts a`, `common_refpoint_of_array (elts, me, a)`:
///   `'()` for none.
/// - `%generic-group-extent me a`, `generic_group_extent`: on Y, each
///   element's `vertical-skylines` read first, but a cross-staff stem's;
///   then the union of the elements' extents relative to their common
///   refpoint with `me`, less `me`'s coordinate there. `::width` and
///   `::height`.
/// - `%item-break-visible? it`, `Item::break_visible`: the
///   `break-visibility` entry of its break direction is `#t`, or there is
///   no vector.
/// - `%grob-pure-visible-piece g start end`,
///   `pure_find_visible_prebroken_piece`: a spanner itself; an item on
///   the column of rank `start` its piece at the beginning of a line, of
///   rank `end` its piece at the end of one, else itself, if that is
///   visible there; `#f` for none. An item without a column has none; an
///   item not broken (before breaking, step 33) has no piece, where
///   LilyPond 2.26.0 dereferences none.
/// - `%relative-pure-height me start end`: an alignment's child's is the
///   union of its heights at the beginning and in the rest of the line
///   (`sum_partial_pure_heights`); another group's, the union of the pure
///   heights relative to its `pure-Y-common` of its `pure-relevant-grobs`
///   visible between `start` and `end` and spanning a column there, but
///   cross-staff stems.
/// - `%part-of-line-pure-height me begin start end`: kept in the
///   spanner's pure cache as `begin-of-line-pure-height` or
///   `rest-of-line-pure-height`; else the car or cdr of its
///   `adjacent-pure-heights` combined between `start` and `end`, `(0 . 0)`
///   where that is no pair or no vector, and for what is no spanner.
///   Reading `adjacent-pure-heights` while it is computed is the cyclic
///   dependency programming error (`grob-property.cc`), `(0 . 0)`: T29's.
/// - `%combine-pure-heights me extents start end`: the union of `extents`'
///   entries from the break at or after `start` to the last break before
///   `end`. LilyPond reads each break's rank as its column's
///   (`cols[breaks[i]]->get_rank ()`), which is the break rank
///   `find_break_indices` keeps beside it.
/// - `%pure-group-height me start end`, `pure_group_height`: the relative
///   pure height less `me`'s pure coordinate on its `pure-Y-common`, read
///   first; LilyPond's programming error, and the empty interval, for no
///   common refpoint. `::pure-height`, `start` and `end` as
///   `from_scm<vsize>` gives them, 0 and `INT_MAX` for what is none.
/// - `%adjacent-pure-heights me`, `adjacent_pure_heights`: for each
///   measure between two break ranks, the union of the pure heights of
///   `me`'s `pure-relevant-grobs` at the beginning of a line and in the
///   rest of one, an outside-staff grob's put beyond the in-staff heights
///   found before the first such grob (`union_disjoint` by its
///   `outside-staff-padding`, 0.46 by default, and its `direction`'s data,
///   up for none or 0). Cross-staff grobs are left out, as dead ones
///   without a column. No break rank gives two empty vectors, where
///   LilyPond 2.26.0 sizes them by `ranks.size () - 1`.
/// - `%axis-group-children me`, `get_children`: `me`, then, if it is an
///   axis group, its `elements`' children, depth first.
/// - `%break-ranks me`, `get_root_system (me)->paper_score ()->
///   get_break_ranks ()`: `paper_score.rs`' (MS3 step 32), empty for no
///   root system or none in a Paper_score.
/// - `::calc-x-common`, `::calc-y-common` (`calc_common`) and
///   `::calc-pure-y-common`: the common refpoint of `me` and its
///   `elements` or `pure-relevant-grobs`, `'()` with LilyPond's
///   programming error for none; a pure one that is an alignment other
///   than `me` is `me`, with the programming error. The first two check no
///   argument, as LilyPond's: what is no grob is `ly:grob-object`'s type
///   error, where LilyPond 2.26.0 dereferences no grob.
pub(crate) const PRELUDE: &str = r#"
(define-public (ly:axis-group-interface::add-element grob grob-element)
  (%check-type "ly:axis-group-interface::add-element" 1 ly:grob? "Grob" grob)
  (%check-type "ly:axis-group-interface::add-element" 2 ly:grob? "Grob" grob-element)
  (let ((axes (ly:grob-property grob 'axes)))
    (if (not (pair? axes))
        (ly:programming-error "axes should be nonempty"))
    (let loop ((p axes))
      (when (pair? p)
        (let ((a (car p)))
          (cond ((not (exact-integer? a))
                 (scm-error 'wrong-type-arg #f "Wrong type (expecting ~A): ~S"
                            (list "exact integer" a) (list a)))
                ((not (memv a '(0 1)))
                 (scm-error 'out-of-range #f "Value out of range ~S to< ~S: ~S"
                            (list 0 1 a) (list a))))
          (if (null? (ly:grob-parent grob-element a))
              (ly:grob-set-parent! grob-element a grob))
          (ly:grob-set-object! grob-element
                               (if (eqv? a 0) 'axis-group-parent-X 'axis-group-parent-Y)
                               grob))
        (loop (cdr p))))
    (ly:pointer-group-interface::add-grob grob 'elements grob-element)))
(define (%axis-group-children me)
  (cons me (if (%grob-has-interface? me 'axis-group-interface)
               (apply append (map %axis-group-children (%grob-set me 'elements)))
               '())))
(define (%interval-unite r d)
  (cons (if (< (car r) (car d)) (car r) (car d))
        (if (< (cdr d) (cdr r)) (cdr r) (cdr d))))
(define (%relative-maybe-bound-group-extent elts common a bound)
  (let loop ((es elts) (r '(+inf.0 . -inf.0)))
    (if (null? es)
        r
        (let ((se (car es)))
          (loop (cdr es)
                (if (eq? (ly:grob-property se 'cross-staff) #t)
                    r
                    (let ((dims (if (and bound (%grob-has-interface? se 'axis-group-interface))
                                    (%generic-bound-extent se common a)
                                    (%grob-extent se common a))))
                      (if (%interval-empty? dims) r (%interval-unite r dims)))))))))
(define (%generic-bound-extent me common a)
  (if (eqv? a 1) (ly:grob-property me 'vertical-skylines))
  (let* ((elts (%grob-set me 'elements))
         (interfaces (ly:grob-property me 'bound-alignment-interfaces))
         (new-elts
          (let outer ((es elts) (acc '()))
            (if (null? es)
                (reverse acc)
                (outer (cdr es)
                       (let inner ((p interfaces) (acc acc))
                         (if (pair? p)
                             (inner (cdr p)
                                    (if (%grob-has-interface? (car es) (car p))
                                        (cons (car es) acc)
                                        acc))
                             acc)))))))
    (if (null? new-elts)
        (%grob-robust-relative-extent me common a)
        (%relative-maybe-bound-group-extent
         new-elts
         (if (null? common)
             (ly:grob-common-refpoint-of-array me (ly:grob-list->grob-array new-elts) a)
             common)
         a #t))))
(define (%pure-relevant-grobs me sym)
  (let loop ((es (%grob-set me sym)) (acc '()))
    (if (null? es)
        (%grob-array-by-priority (reverse acc))
        (let ((g (car es)))
          (loop (cdr es)
                (if (and (ly:item? g) (not (eq? (ly:grob-original g) g)))
                    acc
                    (cons (cons g (%real-or (ly:grob-property g 'outside-staff-priority) -inf.0))
                          acc)))))))
(define-public (ly:relative-group-extent elements common axis)
  (%check-type "ly_relative_group_extent" 1
               (lambda (x) (or (ly:grob-array? x) (list? x))) "list or Grob_array" elements)
  (%check-type "ly:relative-group-extent" 2 ly:grob? "Grob" common)
  (%check-type "ly:relative-group-extent" 3 (lambda (x) (memv x '(0 1))) "axis" axis)
  (%relative-maybe-bound-group-extent
   (ly:grob-array->list (if (ly:grob-array? elements)
                            elements
                            (ly:grob-list->grob-array elements)))
   common axis #f))
(define-public (ly:generic-bound-extent grob common)
  (%check-type "ly:generic-bound-extent" 1 ly:grob? "Grob" grob)
  (%check-type "ly:generic-bound-extent" 2 ly:grob? "Grob" common)
  (%generic-bound-extent grob common 0))
(define (%group-common me elts a)
  (let ((common (ly:grob-common-refpoint-of-array me (ly:grob-list->grob-array elts) a)))
    (if (ly:grob? common) common '())))
(define (%interval-minus r x)
  (cons (- (car r) x) (- (cdr r) x)))
(define (%generic-group-extent me a)
  (let ((elts (%grob-set me 'elements)))
    (if (eqv? a 1)
        (for-each (lambda (e)
                    (if (not (and (%grob-has-interface? e 'stem-interface)
                                  (eq? (ly:grob-property e 'cross-staff) #t)))
                        (ly:grob-property e 'vertical-skylines)))
                  elts))
    (let* ((common (%group-common me elts a))
           (my-coord (%grob-relative-coordinate me common a))
           (r (%relative-maybe-bound-group-extent elts common a #f)))
      (%interval-minus r my-coord))))
(define (%item-break-visible? it)
  (let ((vis (ly:grob-property it 'break-visibility)))
    (if (vector? vis)
        (eq? (vector-ref vis (1+ (ly:item-break-dir it))) #t)
        #t)))
(define (%grob-pure-visible-piece g start end)
  (if (not (ly:item? g))
      g
      (let ((col (ly:item-get-column g)))
        (and (ly:grob? col)
             (let* ((rank (car (ly:grob-spanned-column-rank-interval col)))
                    (it (cond ((= rank start) (%item-prebroken-piece g 1))
                              ((= rank end) (%item-prebroken-piece g -1))
                              (else g))))
               (and it (%item-break-visible? it) it))))))
(define (%pure-y-common me)
  (let ((common (ly:grob-object me 'pure-Y-common)))
    (if (ly:grob? common) common '())))
(define (%relative-pure-height me start end)
  (let ((p (ly:grob-parent me 1)))
    (if (and (ly:grob? p) (%grob-has-interface? p 'align-interface))
        (%interval-unite (%part-of-line-pure-height me #t start (1+ start))
                         (%part-of-line-pure-height me #f start end))
        (let ((common (%pure-y-common me)))
          (let loop ((es (%grob-set me 'pure-relevant-grobs)) (r '(+inf.0 . -inf.0)))
            (if (null? es)
                r
                (let ((g (%grob-pure-visible-piece (car es) start end)))
                  (loop (cdr es)
                        (if (and g
                                 (let ((span (ly:grob-spanned-column-rank-interval g)))
                                   (and (<= (car span) end) (>= (cdr span) start)))
                                 (not (and (eq? (ly:grob-property g 'cross-staff) #t)
                                           (%grob-has-interface? g 'stem-interface))))
                            (let ((dims (%grob-pure-y-extent g common start end)))
                              (if (%interval-empty? dims) r (%interval-unite r dims)))
                            r)))))))))
(define (%part-of-line-pure-height me begin start end)
  (if (%host-waiting?)
      (%scheme-part-of-line-pure-height me begin start end)
      (%host-call 'axis-group 'part-of-line-pure-height me begin start end)))
(define (%scheme-part-of-line-pure-height me begin start end)
  (if (not (ly:spanner? me))
      '(0.0 . 0.0)
      (let* ((sym (if begin 'begin-of-line-pure-height 'rest-of-line-pure-height))
             (cached (%spanner-cached-pure-property me sym start end)))
        (if (pair? cached)
            (if (%number-pair? cached) (%scm->interval cached) '(0.0 . 0.0))
            (let* ((heights (ly:grob-property me 'adjacent-pure-heights))
                   (these (and (pair? heights) (if begin (car heights) (cdr heights))))
                   (ret (if (vector? these)
                            (%combine-pure-heights me these start end)
                            '(0.0 . 0.0))))
              (%spanner-cache-pure-property! me sym start end ret)
              ret)))))
(define (%lower-bound ranks x)
  (let loop ((i 0))
    (if (and (< i (vector-length ranks)) (< (vector-ref ranks i) x)) (loop (1+ i)) i)))
(define (%combine-pure-heights me extents start end)
  (let ((ranks (%break-ranks me)))
    (let loop ((i (%lower-bound ranks start)) (ext '(+inf.0 . -inf.0)))
      (if (and (< (1+ i) (vector-length ranks)) (< (vector-ref ranks i) end))
          (loop (1+ i) (%interval-unite ext (%scm->interval (vector-ref extents i))))
          ext))))
(define (%pure-group-height me start end)
  (let ((common (ly:grob-object me 'pure-Y-common)))
    (if (not (ly:grob? common))
        (begin
          (ly:programming-error "no pure Y common refpoint")
          '(+inf.0 . -inf.0))
        (let* ((my-coord (%grob-pure-relative-y-coordinate me common start end))
               (r (%relative-pure-height me start end)))
          (%interval-minus r my-coord)))))
(define (%union-disjoint iv h padding d)
  (let ((translation (* d (- (+ (if (= d 1) (cdr iv) (car iv)) (* d padding))
                             (if (= d 1) (car h) (cdr h))))))
    (%interval-unite iv (if (> translation 0)
                            (cons (+ (car h) translation) (+ (cdr h) translation))
                            h))))
(define (%adjacent-pure-heights me)
  (let* ((common (%pure-y-common me))
         (elts (%grob-set me 'pure-relevant-grobs))
         (ranks (%break-ranks me))
         (n (vector-length ranks))
         (begin-heights (make-vector (max 0 (1- n)) '(+inf.0 . -inf.0)))
         (mid-heights (make-vector (max 0 (1- n)) '(+inf.0 . -inf.0)))
         (begin-staff-heights #f)
         (mid-staff-heights #f))
    (for-each
     (lambda (g)
       (if (not (or (eq? (ly:grob-property g 'cross-staff) #t)
                    (and (not (%grob-live? g))
                         (not (and (ly:item? g) (ly:grob? (ly:item-get-column g)))))))
           (let* ((outside-staff (number? (ly:grob-property g 'outside-staff-priority)))
                  (padding (%real-or (ly:grob-property g 'outside-staff-padding) 0.46))
                  (d (let ((d (%grob-property-data g 'direction)))
                       (if (and (exact-integer? d) (memv d '(-1 1))) d 1)))
                  (span (ly:grob-spanned-column-rank-interval g))
                  (first-break (max 0 (1- (%lower-bound ranks (car span))))))
             (if (and outside-staff (not begin-staff-heights))
                 (begin
                   (set! begin-staff-heights (vector-copy begin-heights))
                   (set! mid-staff-heights (vector-copy mid-heights))))
             (let loop ((j first-break))
               (if (and (< (1+ j) n) (<= (vector-ref ranks j) (cdr span)))
                   (let* ((start (vector-ref ranks j))
                          (end (vector-ref ranks (1+ j)))
                          (visibility-end (if (< (+ j 2) n) (vector-ref ranks (+ j 2)) end))
                          (subst (%grob-pure-visible-piece g start visibility-end)))
                     (if subst
                         (let ((dims (%grob-pure-y-extent subst common start end)))
                           (if (not (%interval-empty? dims))
                               (begin
                                 (if (<= (car span) start)
                                     (vector-set! begin-heights j
                                                  (%interval-unite
                                                   (vector-ref begin-heights j)
                                                   (if outside-staff
                                                       (%union-disjoint (vector-ref begin-staff-heights j)
                                                                        dims padding d)
                                                       dims))))
                                 (if (> (cdr span) start)
                                     (vector-set! mid-heights j
                                                  (%interval-unite
                                                   (vector-ref mid-heights j)
                                                   (if outside-staff
                                                       (%union-disjoint (vector-ref mid-staff-heights j)
                                                                        dims padding d)
                                                       dims))))))))
                     (loop (1+ j))))))))
     elts)
    (cons begin-heights mid-heights)))
(define-public (ly:axis-group-interface::width smob)
  (%check-type "ly:width" 1 ly:grob? "Grob" smob)
  (%generic-group-extent smob 0))
(define-public (ly:axis-group-interface::height smob)
  (%check-type "ly:height" 1 ly:grob? "Grob" smob)
  (%generic-group-extent smob 1))
(define-public (ly:axis-group-interface::pure-height smob start end)
  (%check-type "ly:pure-height" 1 ly:grob? "Grob" smob)
  (let ((start (%vsize-or start 0))
        (end (%vsize-or end 2147483647)))
    (if (%host-waiting?)
        (%pure-group-height smob start end)
        (%host-call 'axis-group 'pure-height smob start end))))
(define-public (ly:axis-group-interface::calc-pure-relevant-grobs smob)
  (%check-type "ly:calc-pure-relevant-grobs" 1 ly:grob? "Grob" smob)
  (%pure-relevant-grobs smob 'elements))
(define-public (ly:axis-group-interface::calc-pure-y-common smob)
  (%check-type "ly:calc-pure-y-common" 1 ly:grob? "Grob" smob)
  (let* ((common (%group-common smob (%grob-set smob 'pure-relevant-grobs) 1))
         (common (if (and (not (eq? common smob)) (ly:grob? common)
                          (%grob-has-interface? common 'align-interface))
                     (begin
                       (%grob-programming-error
                        smob "My pure_y_common is a VerticalAlignment, which might contain several staves.")
                       smob)
                     common)))
    (if (ly:grob? common)
        common
        (begin
          (%grob-programming-error smob "No common parent found in calc_pure_y_common.")
          '()))))
(define (%calc-common me a)
  (let ((common (%group-common me (%grob-set me 'elements) a)))
    (if (ly:grob? common)
        common
        (begin
          (%grob-programming-error me "No common parent found in calc_common axis.")
          '()))))
(define-public (ly:axis-group-interface::calc-x-common grob)
  (%calc-common grob 0))
(define-public (ly:axis-group-interface::calc-y-common grob)
  (%calc-common grob 1))
(define-public (ly:axis-group-interface::adjacent-pure-heights smob)
  (%check-type "ly:adjacent-pure-heights" 1 ly:grob? "Grob" smob)
  (if (%host-waiting?)
      (%adjacent-pure-heights smob)
      (%host-call 'axis-group 'adjacent-pure-heights smob)))
(define (%skyline-pair? x)
  (and (pair? x) (ly:skyline? (car x)) (ly:skyline? (cdr x))))
(define (%infinite? x) (or (= x +inf.0) (= x -inf.0)))
(define (%outside-staff-ancestor me)
  (let ((parent (ly:grob-parent me 1)))
    (cond ((not (ly:grob? parent)) #f)
          ((number? (ly:grob-property parent 'outside-staff-priority)) parent)
          (else (%outside-staff-ancestor parent)))))
(define (%outside-staff-placement-directive me)
  (let ((directive (ly:grob-property me 'outside-staff-placement-directive)))
    (if (memq directive '(left-to-right-greedy left-to-right-polite
                          right-to-left-greedy right-to-left-polite))
        directive
        (begin
          (%grob-warning me (string-append
                             "\"" (if (symbol? directive) (symbol->string directive) "")
                             "\" is not a valid outside-staff-placement-directive"))
          'left-to-right-polite))))
(define (%add-grobs-of-one-priority me placed elements x-common y-common riders)
  (let* ((directive (%outside-staff-placement-directive me))
         (l2r (memq directive '(left-to-right-greedy left-to-right-polite)))
         (polite (memq directive '(left-to-right-polite right-to-left-polite))))
    (let pass ((elements elements))
      (if (pair? elements)
          (let ((last-end (vector -inf.0 -inf.0)))
            (let loop ((es (if l2r elements (reverse elements))) (skipped '()))
              (if (null? es)
                  (pass (reverse! skipped))
                  (let* ((elt (car es))
                         (padding (%real-or (ly:grob-property elt 'outside-staff-padding) 0.46))
                         (horizon-padding
                          (%real-or (ly:grob-property elt 'outside-staff-horizontal-padding) 0.0))
                         (x-extent (%grob-extent elt x-common 0))
                         (x-extent (cons (- (car x-extent) horizon-padding)
                                         (+ (cdr x-extent) horizon-padding)))
                         (dir (let ((d (ly:grob-property elt 'direction)))
                                (if (and (exact-integer? d) (memv d '(-1 0 1))) d 0)))
                         (dir (if (eqv? dir 0)
                                  (begin
                                    (ly:warning "an outside-staff object should have a direction, defaulting to up")
                                    1)
                                  dir))
                         (side (if (eqv? dir 1) 1 0)))
                    (if (and polite (<= (car x-extent) (vector-ref last-end side)))
                        (loop (cdr es) (cons elt skipped))
                        (let ((orig (begin (vector-set! last-end side (cdr x-extent))
                                           (ly:grob-property elt 'vertical-skylines))))
                          (if (or (not (%skyline-pair? orig)) (%skyline-pair-empty? orig))
                              (loop (cdr es) skipped)
                              (let* ((rider-skylines
                                      (let rl ((rs riders) (out '()))
                                        (cond ((null? rs) (reverse! out))
                                              ((not (eq? (caar rs) elt)) (rl (cdr rs) out))
                                              (else
                                               (let* ((rider (cdar rs))
                                                      (v (ly:grob-property rider 'vertical-skylines)))
                                                 (rl (cdr rs)
                                                     (if (%skyline-pair? v)
                                                         (let* ((v (%skyline-pair-shifted
                                                                    v (%grob-relative-coordinate rider x-common 0)))
                                                                (v (%skyline-pair-raised
                                                                    v (%grob-relative-coordinate rider y-common 1))))
                                                           (cons v out))
                                                         out)))))))
                                     (v (%skyline-pair-shifted orig (%grob-relative-coordinate elt x-common 0)))
                                     (v (%skyline-pair-raised v (%grob-relative-coordinate elt y-common 1)))
                                     (v (%skyline-pairs-merged v (list (%skyline-pairs-union rider-skylines))))
                                     (others (vector-ref placed side))
                                     (move (%outside-staff-move v padding horizon-padding
                                                                (reverse (car others))
                                                                (reverse (cadr others))
                                                                (reverse (caddr others))
                                                                dir))
                                     (v (%skyline-pair-raised v move)))
                                (ly:grob-translate-axis! elt move 1)
                                (%grob-set-property-data! elt 'outside-staff-priority #f)
                                (%queue-check 15)
                                (vector-set! placed side
                                             (list (cons v (car others))
                                                   (cons padding (cadr others))
                                                   (cons horizon-padding (caddr others))))
                                (loop (cdr es) skipped)))))))))))))
(define (%skyline-spacing me)
  (let* ((orig (%grob-set me (if (ly:grob-array? (ly:grob-object me 'vertical-skyline-elements))
                                 'vertical-skyline-elements
                                 'elements)))
         (x-common (%group-common me orig 0))
         (y-common (if (eq? (%group-common me orig 1) me)
                       me
                       (begin
                         (%grob-programming-error
                          me "Some of my vertical-skyline-elements are outside my VerticalAxisGroup.")
                         me)))
         (keys (let loop ((gs orig) (out '()))
                 (if (null? gs)
                     (reverse! out)
                     (let* ((g (car gs))
                            (priority (%real-or (ly:grob-property g 'outside-staff-priority) -inf.0)))
                       (loop (cdr gs)
                             (cons (cond ((%infinite? priority) (list g priority 0))
                                         ((%outside-staff-ancestor g)
                                          (%grob-warning
                                           g "Cannot set outside-staff-priority for element and elements' Y parent.")
                                          (%grob-set-property-data! g 'outside-staff-priority #f)
                                          (list g -inf.0 0))
                                         (else (list g priority (car (%grob-extent g x-common 0)))))
                                   out))))))
         (keys (stable-sort keys (lambda (k1 k2)
                                   (if (not (= (cadr k1) (cadr k2)))
                                       (< (cadr k1) (cadr k2))
                                       (< (caddr k1) (caddr k2)))))))
    (let inside ((ks keys) (riders '()) (skys '()))
      (if (and (pair? ks) (%infinite? (cadar ks)))
          (let* ((elt (caar ks))
                 (ancestor (%outside-staff-ancestor elt)))
            (cond (ancestor (inside (cdr ks) (cons (cons ancestor elt) riders) skys))
                  ((eq? (ly:grob-property elt 'cross-staff) #t) (inside (cdr ks) riders skys))
                  (else
                   (let ((skyp (ly:grob-property elt 'vertical-skylines)))
                     (if (or (not (%skyline-pair? skyp)) (%skyline-pair-empty? skyp))
                         (inside (cdr ks) riders skys)
                         (let* ((skyp (%skyline-pair-shifted skyp (%grob-relative-coordinate elt x-common 0)))
                                (skyp (%skyline-pair-raised skyp (%grob-relative-coordinate elt y-common 1))))
                           (inside (cdr ks) riders (cons skyp skys))))))))
          (let* ((riders (reverse! riders))
                 (skylines (%skyline-pairs-union (reverse! skys)))
                 ;; For DOWN and UP: the skylines placed, their paddings and
                 ;; their horizon paddings, each list last first.
                 (placed (vector (list (list skylines) '(0.0) '(0.0))
                                 (list (list skylines) '(0.0) '(0.0)))))
            (let outside ((ks ks))
              (cond ((null? ks) #t)
                    ((eq? (ly:grob-property (caar ks) 'cross-staff) #t) (outside (cdr ks)))
                    (else
                     (let collect ((ks ks) (current (list (caar ks))))
                       (if (and (pair? (cdr ks)) (= (cadar ks) (cadadr ks)))
                           (collect (cdr ks)
                                    (if (eq? (ly:grob-property (caadr ks) 'cross-staff) #t)
                                        current
                                        (cons (caadr ks) current)))
                           (begin
                             (%add-grobs-of-one-priority me placed (reverse! current)
                                                         x-common y-common riders)
                             (outside (cdr ks))))))))
            (let* ((other (%skyline-pairs-union (reverse (car (vector-ref placed 1)))))
                   (other (%skyline-pairs-merged
                           other (list (%skyline-pairs-union (reverse (car (vector-ref placed 0)))))))
                   (skylines (%skyline-pairs-merged skylines (list other))))
              (%skyline-pair-shifted skylines (- (%grob-relative-coordinate me x-common 0)))))))))
(define-public (ly:axis-group-interface::calc-skylines smob)
  (%check-type "ly:calc-skylines" 1 ly:grob? "Grob" smob)
  (%skyline-spacing smob))
(define-public (ly:axis-group-interface::combine-skylines smob)
  (%check-type "ly:combine-skylines" 1 ly:grob? "Grob" smob)
  (let* ((elements (%grob-set smob 'elements))
         (y-common (%group-common smob elements 1))
         (x-common (%group-common smob elements 0)))
    (if (not (eq? y-common smob))
        (ly:programming-error "combining skylines that don't belong to me"))
    (let loop ((es elements) (ret (%skyline-pairs-merged #f '())))
      (if (null? es)
          (%skyline-pair-shifted ret (- (%grob-relative-coordinate smob x-common 0)))
          (let ((skyp (ly:grob-property (car es) 'vertical-skylines)))
            (loop (cdr es)
                  (if (%skyline-pair? skyp)
                      (let* ((offset (%grob-relative-coordinate (car es) y-common 1))
                             (skyp (%skyline-pair-raised skyp offset))
                             (skyp (%skyline-pair-shifted
                                    skyp (%grob-relative-coordinate (car es) x-common 0))))
                        (%skyline-pairs-merged ret (list skyp)))
                      ret)))))))
(define (%maybe-pure-staff-staff-spacing me pure start end)
  (let* ((grouper (ly:grob-object me 'staff-grouper))
         (g (if (ly:grob? grouper) grouper me))
         (sym (cond ((not (ly:grob? grouper)) 'default-staff-staff-spacing)
                    ((%staff-grouper-within-group? grouper me pure start end) 'staff-staff-spacing)
                    (else 'staffgroup-staff-spacing))))
    (if pure
        (%grob-pure-property g sym start end)
        (ly:grob-property g sym))))
(define-public (ly:axis-group-interface::calc-pure-staff-staff-spacing smob start end)
  (let* ((start (%scm->vsize start))
         (end (%scm->vsize end)))
    (%maybe-pure-staff-staff-spacing smob #t start end)))
(define-public (ly:axis-group-interface::calc-staff-staff-spacing smob)
  (%maybe-pure-staff-staff-spacing smob #f 0 2147483647))
"#;
