//! `grob-pq-engraver.cc` (MS3 step 21): `Grob_pq_engraver`, a class of
//! layout's ([`GROB_PQ_ENGRAVER`]) that the core passes at its `\layout`
//! call and that a group finds before the music crate's placeholder of the
//! name (D67, D108). It keeps `busyGrobs`, the grobs still playing as an
//! alist of (end . grob) by their ends, which no class of the engine reads
//! yet.
//!
//! - `initialize` (`:62–66`): `busyGrobs` `'()` in its context.
//! - Each grob acknowledged (`:81–95`) with an event cause
//!   (`tie_engraver::event_cause`) and without `multi-measure-interface`
//!   is kept with its end, now and its event's length
//!   (`tie_engraver::event_length`, `get_event_length`), if that is not
//!   zero.
//! - `process_acknowledged` (`:97–115`): the kept grobs sorted by their
//!   ends and merged into `busyGrobs` as `scm_merge_x` does with
//!   `ly:grob-pq<?` (`:68–79`), here a comparison of the ends by
//!   `Moment::compare` ([`merge_x`]); LilyPond's `ly:grob-pq<?` is not
//!   defined. The sort is libc++'s `std::sort`
//!   ([`lily_engine_skyline::sort`], MS3 step 60, OPEN A14), which is not
//!   stable: grobs with equal ends are in its order, as LilyPond's are.
//! - `start_translation_timestep` (`:127–145`): the leading entries that
//!   end before now dropped, `busyGrobs` set only if any was. As it must
//!   run at every timestep, the class runs always.
//!
//! `stop_translation_timestep` (`:117–125`) walks the entries that end now
//! and changes nothing, so it is left out.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;

use lily_engine_music::{
    INITIALIZE, Kind, Moment, PROCESS_ACKNOWLEDGED, Rational, START, TranslatorClass, Tx, Work,
};
use lily_engine_scheme::{Exception, Heap, Value};

use crate::grob;
use crate::tie_engraver::{event_cause, event_length};

type Run<T> = Result<T, Exception>;

pub static GROB_PQ_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Grob_pq_engraver",
    kind: Kind::Engraver,
    methods: INITIALIZE | START | PROCESS_ACKNOWLEDGED,
    // `start_translation_timestep` drops the ended grobs every timestep.
    always: true,
    hears: &[],
    acknowledges: &[("grob-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(GrobPqEngraver::default())),
    reads: &["busyGrobs"],
    writes: &["busyGrobs"],
};

/// `started_now_`: the grobs acknowledged this timestep by their handles,
/// with their ends (`Grob_pq_entry`).
#[derive(Clone, Default)]
struct GrobPqEngraver {
    started_now: Vec<(Moment, Value)>,
}

/// The end of a `busyGrobs` entry, `None` if it is not a moment.
fn end_of(heap: &Heap, entry: Value) -> Option<Moment> {
    let end = heap.car(entry)?;
    heap.foreign::<Moment>(end).copied()
}

/// `ly:grob-pq<?`: whether entry `a` ends before entry `b`.
fn pq_less(heap: &Heap, a: Value, b: Value) -> bool {
    match (end_of(heap, a), end_of(heap, b)) {
        (Some(a), Some(b)) => a.compare(b) == Ordering::Less,
        _ => false,
    }
}

/// `scm_merge_x (a, b, ly:grob-pq<?)`: the pairs of `a` and `b` relinked
/// into one list, taking `b`'s head only where it is less than `a`'s, so
/// that at equal ends `a`'s entries come first.
fn merge_x(heap: &mut Heap, mut a: Value, mut b: Value) -> Value {
    let mut pairs = Vec::new();
    let rest = loop {
        let (Some(x), Some(y)) = (heap.car(a), heap.car(b)) else {
            break if heap.car(a).is_some() { a } else { b };
        };
        if pq_less(heap, y, x) {
            pairs.push(b);
            b = heap.cdr(b).unwrap_or(Value::NULL);
        } else {
            pairs.push(a);
            a = heap.cdr(a).unwrap_or(Value::NULL);
        }
    };
    let mut next = rest;
    for &p in pairs.iter().rev() {
        heap.set_cdr(p, next);
        next = p;
    }
    next
}

impl Work for GrobPqEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.num(self.started_now.len() as i64);
        for &(m, _) in &self.started_now {
            h.moment(m);
        }
    }

    fn initialize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        tx.set("busyGrobs", Value::NULL);
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        let Some(ev) = event_cause(tx, g)? else {
            return Ok(());
        };
        if grob::has_interface(tx.heap(), g, "multi-measure-interface") {
            return Ok(());
        }
        let now = tx.now();
        let len = event_length(tx.heap(), ev, now);
        if !(len.main.is_zero() && len.grace.is_zero()) {
            let end = now
                .checked_add(len)
                .unwrap_or(Moment::new(Rational::INFINITY, Rational::ZERO));
            self.started_now.push((end, g));
        }
        Ok(())
    }

    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        lily_engine_skyline::sort(
            &mut self.started_now,
            &|a: &(Moment, Value), b: &(Moment, Value)| a.0.compare(b.0) == Ordering::Less,
        );
        let heap = tx.heap();
        let mut lst = Value::NULL;
        for &(end, g) in self.started_now.iter().rev() {
            let end = heap.wrap(end);
            let entry = heap.cons(end, g);
            lst = heap.cons(entry, lst);
        }

        let busy = tx.get("busyGrobs");
        let busy = merge_x(tx.heap(), lst, busy);
        tx.set("busyGrobs", busy);

        self.started_now.clear();
        Ok(())
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let now = tx.now();

        let start_busy = tx.get("busyGrobs");
        let mut busy = start_busy;
        let heap = tx.heap();
        while let (Some(entry), Some(rest)) = (heap.car(busy), heap.cdr(busy)) {
            if !end_of(heap, entry).is_some_and(|end| end.compare(now) == Ordering::Less) {
                break;
            }
            // The grob-pq-engraver is not water tight, and stuff like
            // tupletSpannerDuration confuses it.
            busy = rest;
        }

        if start_busy != busy {
            tx.set("busyGrobs", busy);
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.started_now.iter().map(|&(_, g)| g));
    }
}
