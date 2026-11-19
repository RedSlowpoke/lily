//! `cluster-engraver.cc` and `cluster.cc` (MS8 step 8, part 2; 171 and
//! 257 lines): `Cluster_spanner_engraver`, a class of layout's
//! ([`CLUSTER_SPANNER_ENGRAVER`]) that shadows the music crate's
//! placeholder of its name (D67, D108), and `ly:cluster::print`,
//! `ly:cluster::calc-cross-staff` and `ly:cluster-beacon::height`, Rust
//! under primitives that print as LilyPond's subrs and call it over
//! `%host-call` (D127, [`HOSTS`]).
//!
//! - The engraver: the timestep's `cluster-note-event`s make a
//!   `ClusterSpannerBeacon` of their lowest and highest staff positions,
//!   one of the running `ClusterSpanner`'s `columns` and bounds (made at
//!   the first); a note column without a beacon ends the spanner, its right
//!   bound its left where it has none.
//! - `print`: the columns' Y extents as bottom and top points, with the
//!   next broken piece's first column's at a line break, drawn by `style`
//!   (`leftsided-stairs`, `rightsided-stairs`, `centered-stairs`, `ramp`;
//!   LilyPond's warning for another), padded by `padding`; an empty cluster
//!   junked with LilyPond's warning.
//! - `calc-cross-staff`: whether the columns' common Y refpoint is not the
//!   cluster's Y parent. The beacon's height: its `positions` in half staff
//!   spaces.

use alloc::boxed::Box as Boxed;
use alloc::format;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, Host, Kind, PROCESS, Pitch, STOP, TranslatorClass, Tx, Work, event_property,
};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::geometry::{Box, Interval, Offset, X, Y};
use crate::note_heads_engraver::steps;
use crate::spacing_options::double_or;
use crate::stencil::Stencil;
use crate::{grob, grob_array, lookup, spanner};

type Run<T> = Result<T, Exception>;

pub static CLUSTER_SPANNER_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Cluster_spanner_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP | FINALIZE,
    always: false,
    hears: &["cluster-note-event"],
    acknowledges: &[("note-column-interface", false)],
    must_be_last: false,
    make: Some(|| Boxed::new(ClusterSpannerEngraver::default())),
    reads: &["middleCPosition"],
    writes: &[],
};

/// `cluster_notes_`, and `beacon_`, `spanner_` and `finished_spanner_` by
/// their handles (the spanners with their record numbers).
#[derive(Clone, Default)]
struct ClusterSpannerEngraver {
    cluster_notes: Vec<Value>,
    beacon: Option<Value>,
    spanner: Option<(Value, u32)>,
    finished_spanner: Option<(Value, u32)>,
}

impl ClusterSpannerEngraver {
    fn typeset_grobs(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some((finished, _)) = self.finished_spanner.take()
            && spanner::get_bound(tx.host, finished, 1)?.is_none()
            && let Some(left) = spanner::get_bound(tx.host, finished, -1)?
        {
            spanner::set_bound(tx.host, finished, 1, left)?;
        }
        self.beacon = None;
        Ok(())
    }
}

impl Work for ClusterSpannerEngraver {
    fn listen(&mut self, _: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        self.cluster_notes.push(ev);
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(&first) = self.cluster_notes.first() {
            let c0scm = tx.get("middleCPosition");
            let c0 = if tx.heap().is_number(c0scm) {
                crate::stem::to_int(tx.heap(), c0scm)?
            } else {
                0
            };
            let mut pmax = i32::MIN;
            let mut pmin = i32::MAX;
            for &ev in &self.cluster_notes {
                let heap = tx.heap();
                let pitch = event_property(heap, ev, "pitch");
                let pit = heap.foreign::<Pitch>(pitch).copied();
                let p = pit.map_or(0, |p| steps(heap, &p)).wrapping_add(c0);
                pmax = pmax.max(p);
                pmin = pmin.min(p);
            }
            let id = tx.make_defined_grob("ClusterSpannerBeacon", Cause::Event(first))?;
            let beacon = tx.grob_handle(id);
            self.beacon = Some(beacon);
            let heap = tx.heap();
            let (lo, hi) = (heap.integer(i64::from(pmin)), heap.integer(i64::from(pmax)));
            let positions = heap.cons(lo, hi);
            grob::set_property(heap, beacon, "positions", positions);
        }

        if self.beacon.is_some()
            && self.spanner.is_none()
            && let Some(&first) = self.cluster_notes.first()
        {
            let id = tx.make_defined_grob("ClusterSpanner", Cause::Event(first))?;
            self.spanner = Some((tx.grob_handle(id), id));
        }

        if let (Some(beacon), Some((sp, _))) = (self.beacon, self.spanner) {
            spanner::add_bound_item(tx.host, sp, beacon)?;
            let sym = tx.heap().intern("columns");
            grob_array::add_grob(tx.host, sp, sym, beacon)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.typeset_grobs(tx)?;
        self.cluster_notes.clear();
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.typeset_grobs(tx)?;
        self.finished_spanner = self.spanner.take();
        self.typeset_grobs(tx)
    }

    fn acknowledge(&mut self, _: &mut Tx<'_>, _: usize, _: u32) -> Run<()> {
        if self.beacon.is_none() {
            self.finished_spanner = self.spanner.take();
        }
        Ok(())
    }

    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.maybe_record(self.spanner.map(|s| s.1));
        h.maybe_record(self.finished_spanner.map(|s| s.1));
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(&self.cluster_notes);
        out.extend(self.beacon);
        out.extend(self.spanner.map(|s| s.0));
        out.extend(self.finished_spanner.map(|s| s.0));
    }
}

// --- the callbacks ------------------------------------------------------------

fn sub(a: Offset, b: Offset) -> Offset {
    (a.0 - b.0, a.1 - b.1)
}

fn add(a: Offset, b: Offset) -> Offset {
    (a.0 + b.0, a.1 + b.1)
}

/// `brew_cluster_piece`, its expression rooted.
fn brew_cluster_piece(
    host: &mut dyn Host,
    me: Value,
    bottom_points: &[Offset],
    top_points: &[Offset],
) -> Result<Stencil, Exception> {
    let blotdiameter = crate::staff_symbol_referencer::staff_space(host, me)? / 2.0;
    let padding = grob::property(host, me, "padding")?;
    let padding = double_or(host, padding, 0.0);
    let vpadding = (0.0, padding);
    let hpadding = (0.5 * blotdiameter, 0.0);
    let hvpadding = add((0.5 * hpadding.0, 0.5 * hpadding.1), vpadding);
    let shape_scm = grob::property(host, me, "style")?;
    let Some(shape) = host.heap().name(shape_scm).map(alloc::string::String::from) else {
        let text = host
            .heap()
            .string("ClusterSpanner.style should be defined as a symbol.");
        crate::call_lily(host, "ly:programming-error", &[text])?;
        grob::suicide(host.heap(), me);
        return Ok(Stencil::default());
    };

    let mut out = Stencil::default();
    let mut warnings = Vec::new();
    let size = bottom_points.len();
    let mut boxed = |host: &mut dyn Host, out: &mut Stencil, a: Offset, b: Offset| {
        let mut bx = Box::EMPTY;
        bx.add_point(a);
        bx.add_point(b);
        let s = lookup::round_filled_box(host.heap(), bx, blotdiameter, &mut warnings);
        out.add_stencil(host.heap(), &s);
        host.heap().root(out.expr);
    };
    if size == 0 {
        let text = host.heap().string("no points provided");
        crate::call_lily(host, "ly:programming-error", &[text])?;
    } else if shape == "leftsided-stairs" {
        for i in 0..size - 1 {
            boxed(
                host,
                &mut out,
                sub(bottom_points[i], hvpadding),
                add((top_points[i + 1].0, top_points[i].1), hvpadding),
            );
        }
    } else if shape == "rightsided-stairs" {
        for i in 0..size - 1 {
            boxed(
                host,
                &mut out,
                sub((bottom_points[i].0, bottom_points[i + 1].1), hvpadding),
                add(top_points[i + 1], hvpadding),
            );
        }
    } else if shape == "centered-stairs" {
        let mut left_xmid = bottom_points[0].0;
        for i in 0..size - 1 {
            let right_xmid = 0.5 * (bottom_points[i].0 + bottom_points[i + 1].0);
            boxed(
                host,
                &mut out,
                sub((left_xmid, bottom_points[i].1), hvpadding),
                add((right_xmid, top_points[i].1), hvpadding),
            );
            left_xmid = right_xmid;
        }
        let right_xmid = bottom_points[size - 1].0;
        boxed(
            host,
            &mut out,
            sub((left_xmid, bottom_points[size - 1].1), hvpadding),
            add((right_xmid, top_points[size - 1].1), hvpadding),
        );
    } else if shape == "ramp" {
        let mut points = Vec::new();
        points.push(add(sub(bottom_points[0], vpadding), hpadding));
        for &p in bottom_points.iter().take(size - 1).skip(1) {
            points.push(sub(p, vpadding));
        }
        points.push(sub(sub(bottom_points[size - 1], vpadding), hpadding));
        points.push(sub(add(top_points[size - 1], vpadding), hpadding));
        if size >= 2 {
            for i in (1..=size - 2).rev() {
                points.push(add(top_points[i], vpadding));
            }
        }
        points.push(add(add(top_points[0], vpadding), hpadding));
        let s = lookup::round_polygon(host.heap(), &points, blotdiameter, -1.0, true);
        out.add_stencil(host.heap(), &s);
        host.heap().root(out.expr);
    } else {
        grob::warning(host, me, &format!("unknown cluster style `{shape}'"))?;
    }
    for text in &warnings {
        grob::unlocated_warning(host, text)?;
    }
    Ok(out)
}

/// `Cluster::calc_cross_staff`, as above.
pub fn calc_cross_staff(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let cols = grob_array::extract_grob_array(host, me, "columns")?;
    let heap = host.heap();
    let commony = grob::common_refpoint_of_array(heap, &cols, Some(me), Y);
    Ok(Value::bool(commony != grob::get_parent(heap, me, Y)))
}

/// `Cluster::print`, as above.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let left_bound = spanner::get_bound(host, me, -1)?;
    let right_bound = spanner::get_bound(host, me, 1)?;
    // LilyPond's reads through a missing bound.
    let (Some(left_bound), Some(right_bound)) = (left_bound, right_bound) else {
        return Ok(Value::NULL);
    };
    let heap = host.heap();
    let commonx = grob::common_refpoint(heap, left_bound, right_bound, X);
    let cols = grob_array::extract_grob_array(host, me, "columns")?;
    if cols.is_empty() {
        grob::warning(host, me, "junking empty cluster")?;
        grob::suicide(host.heap(), me);
        return Ok(Value::NULL);
    }
    let heap = host.heap();
    let commonx = grob::common_refpoint_of_array(heap, &cols, commonx, X).unwrap_or(me);
    let commony = grob::common_refpoint_of_array(heap, &cols, Some(me), Y).unwrap_or(me);
    let mut bottom_points = Vec::new();
    let mut top_points = Vec::new();

    let left_coord = grob::relative_coordinate(host, left_bound, commonx, X)?;

    // TODO: should we move the cluster a little to the right to be in line
    // with the center of the note heads?
    for &col in &cols {
        let yext = grob::extent(host, col, commony, Y)?;
        let x = grob::relative_coordinate(host, col, commonx, X)? - left_coord;
        bottom_points.push((x, yext.left));
        top_points.push((x, yext.right));
    }

    // Across a line break we anticipate on the next pitches.
    if let Some(next) = spanner::broken_neighbor(host.heap(), me, 1) {
        let next_cols = grob_array::extract_grob_array(host, next, "columns")?;
        if let Some(&col) = next_cols.first() {
            let heap = host.heap();
            let next_commony =
                grob::common_refpoint_of_array(heap, &next_cols, Some(next), Y).unwrap_or(next);
            let v = grob::extent(host, col, next_commony, Y)?;
            let x = grob::relative_coordinate(host, right_bound, commonx, X)? - left_coord;
            bottom_points.push((x, v.left));
            top_points.push((x, v.right));
        }
    }

    let mut out = brew_cluster_piece(host, me, &bottom_points, &top_points)?;
    let y = grob::relative_coordinate(host, me, commony, Y)?;
    out.translate_axis(host.heap(), -y, Y);
    Ok(host.heap().wrap(out))
}

/// `Cluster_beacon::height`, as above.
pub fn beacon_height(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let v = grob::property(host, me, "positions")?;
    let v = grob::number_pair(host.heap(), v).unwrap_or(Interval::new(0.0, 0.0));
    let ss = crate::staff_symbol_referencer::staff_space(host, me)?;
    let heap = host.heap();
    let (l, r) = (
        heap.flonum(ss * (0.5 * v.left)),
        heap.flonum(ss * (0.5 * v.right)),
    );
    Ok(heap.cons(l, r))
}

/// What `%host-call` runs for the `ly:cluster::` and `ly:cluster-beacon::`
/// callbacks (D127).
pub(crate) const HOSTS: [HostHandler; 2] = [
    HostHandler {
        what: "cluster",
        f: host_cluster,
    },
    HostHandler {
        what: "cluster-beacon",
        f: host_beacon,
    },
];

/// The `ly:cluster::` callbacks, by name.
pub(crate) const CALLBACKS: [(&str, Callback); 2] =
    [("calc-cross-staff", calc_cross_staff), ("print", print)];

/// The `ly:cluster-beacon::` callback, by name.
pub(crate) const BEACON_CALLBACKS: [(&str, Callback); 1] = [("height", beacon_height)];

fn host_cluster(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such cluster callback")
}

fn host_beacon(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(
        vm,
        &BEACON_CALLBACKS,
        args,
        "no such cluster-beacon callback",
    )
}

type R = Result<Value, lily_engine_scheme::Thrown>;

pub(crate) fn ly_print(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(cx, "cluster", "ly:cluster::", "print", args)
}

pub(crate) fn ly_calc_cross_staff(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(cx, "cluster", "ly:cluster::", "calc-cross-staff", args)
}

pub(crate) fn ly_beacon_height(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(cx, "cluster-beacon", "ly:cluster-beacon::", "height", args)
}
