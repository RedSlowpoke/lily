//! `accidental-placement.cc` (part 3's, MS3): `add_accidental`
//! (`accidental-placement.cc:55–81`, 27 lines) and the `accidental_pitch`
//! it calls (`:41–53`, 13), which `Accidental_engraver` calls, ported
//! first (MS3 step 17); and `split_accidentals` (`:83–100`, 18) and
//! `get_relevant_accidentals` (`:102–121`, 20), which
//! `Separation_item::conditional_skyline` calls, ported first (MS3 step
//! 30). Then the accidentals' placement over the note columns' skylines
//! (MS3 step 39): `ly:accidental-placement::calc-positioning-done`
//! (`:479–518`, 40), the music crate's placeholder until then (D67), with
//! the entries' `ape_priority` and `ape_less` (`:123–146`, 24),
//! `acc_less` (`:160–181`, 22), `stagger_apes` (`:191–235`, 45),
//! `build_apes` (`:237–251`, 15), `set_ape_skylines` (`:253–301`, 49),
//! `extract_heads_and_stems` (`:303–355`, 53),
//! `common_refpoint_of_accidentals` (`:357–373`, 17),
//! `build_heads_skyline` (`:375–385`, 11) and `position_apes`
//! (`:391–438`, 48): Rust under a primitive that prints as LilyPond's subr
//! and calls it over `%host-call` (D127, [`HOSTS`]), as the stem's do.
//!
//! LilyPond sorts with `std::sort`, which is not stable: the port sorts
//! with libc++'s ([`lily_engine_skyline::sort`]). `acc_less` reads each
//! accidental's pitch at each comparison, and gives LilyPond's
//! programming errors there; the port reads each pitch once, before the
//! sort, and gives "these accidentals do not have a pitch" as often as a
//! comparison meets one without. Where LilyPond would read through a
//! null (an accidental or a head without a parent, an entry that is no
//! pair), the port passes the grob over.

use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use core::cell::Cell;
use core::cmp::Ordering;

use lily_engine_music::{Host, Pitch, Rational, StreamEvent, event_property, grob_of};
use lily_engine_scheme::{BigInt, Cx, Exception, Heap, HostHandler, Value, Vm};
use lily_engine_skyline::{Axis as Horizon, Direction as Sky, Skyline, SkylinePair};

use crate::geometry::{Axis, Interval, X, Y};
use crate::grob;
use crate::grob_array;
use crate::item;
use crate::rhythmic_head;
use crate::skylines;
use crate::spacing_options::double_or;

/// `accidental_pitch`: the pitch of the event that caused the accidental
/// `acc`'s Y parent (`Grob::event_cause`), else LilyPond's programming
/// error. An accidental without a Y parent, on which LilyPond would crash,
/// is taken as one whose parent has no cause.
pub(crate) fn accidental_pitch(
    host: &mut dyn Host,
    acc: Value,
) -> Result<Option<Pitch>, Exception> {
    let cause = match grob::get_parent(host.heap(), acc, Y) {
        Some(parent) => grob::property(host, parent, "cause")?,
        None => Value::NULL,
    };
    let heap = host.heap();
    if heap.foreign::<StreamEvent>(cause).is_none() {
        grob::unlocated_programming_error(host, "note head has no event cause")?;
        return Ok(None);
    }
    let pitch = event_property(heap, cause, "pitch");
    Ok(heap.foreign::<Pitch>(pitch).copied())
}

/// `add_accidental`: the accidental `a` made a child of the placement
/// `me` on X and added in front of the grobs of its key in `me`'s
/// `accidental-grobs`, an alist whose key is its pitch's notename and, for
/// `stagger`, the number of the translator `hash_key`, else 1; a new key's
/// entry put in front. Nothing for an accidental without a pitch.
///
/// The number stands for `std::hash` of the translator's pointer: the
/// word of `hash_key`, its handle, which is one per translator and does
/// not move. Only grouping by it matters: what reads the alist reads only
/// its grobs.
pub fn add_accidental(
    host: &mut dyn Host,
    me: Value,
    a: Value,
    stagger: bool,
    hash_key: Value,
) -> Result<(), Exception> {
    let Some(p) = accidental_pitch(host, a)? else {
        return Ok(());
    };
    grob::set_parent(host.heap(), a, me, X);
    let sym = host.heap().intern("accidental-grobs");
    let accs = grob_array::object(host, me, sym)?;
    let heap = host.heap();
    let hash = if stagger {
        heap.integer_big(BigInt::from_u64(hash_key.to_bits()))
    } else {
        Value::fixnum(1).unwrap()
    };
    let key = heap.cons(Value::fixnum(i64::from(p.notename)).unwrap(), hash);
    let accs = match assoc(heap, key, accs) {
        Some(entry) => {
            let grobs = heap.cdr(entry).unwrap_or(Value::NULL);
            let grobs = heap.cons(a, grobs);
            heap.set_cdr(entry, grobs);
            accs
        }
        None => {
            let grobs = heap.list(&[a]);
            let entry = heap.cons(key, grobs);
            heap.cons(entry, accs)
        }
    };
    grob::set_object(heap, me, sym, accs);
    Ok(())
}

/// `ly_assoc` of a pair, `scm_assoc` (`equal?`), as `scm_assoc_set_x`
/// finds it: the first entry of `key`, what is no pair skipped.
pub(crate) fn assoc(heap: &Heap, key: Value, alist: Value) -> Option<Value> {
    let mut at = alist;
    while let (Some(entry), Some(next)) = (heap.car(at), heap.cdr(at)) {
        if heap.car(entry).is_some_and(|k| heap.equal(k, key)) {
            return Some(entry);
        }
        at = next;
    }
    None
}

/// `split_accidentals`: the grobs of the placement `accs`' `accidental-grobs`
/// added to `break_reminder` if they have a grob as their `tie` and their
/// `forced` is not `#t`, else to `real_acc`. An entry that is no pair, or
/// a list item that is no grob, is passed over where LilyPond would crash.
fn split_accidentals(
    host: &mut dyn Host,
    accs: Value,
    break_reminder: &mut Vec<Value>,
    real_acc: &mut Vec<Value>,
) -> Result<(), Exception> {
    let sym = host.heap().intern("accidental-grobs");
    let mut entries = grob_array::object(host, accs, sym)?;
    while let (Some(entry), Some(rest)) = (host.heap().car(entries), host.heap().cdr(entries)) {
        let mut grobs = host.heap().cdr(entry).unwrap_or(Value::NULL);
        while let (Some(a), Some(more)) = (host.heap().car(grobs), host.heap().cdr(grobs)) {
            if grob::with_grob(host.heap(), a, |_| ()).is_some() {
                let sym = host.heap().intern("tie");
                let tie = grob_array::object(host, a, sym)?;
                let tied = grob::with_grob(host.heap(), tie, |_| ()).is_some();
                if tied && grob::property(host, a, "forced")? != Value::TRUE {
                    break_reminder.push(a);
                } else {
                    real_acc.push(a);
                }
            }
            grobs = more;
        }
        entries = rest;
    }
    Ok(())
}

/// `get_relevant_accidentals`: the accidentals of the placements `elts`
/// printed after `left`, the break reminders too if `left` begins a line.
/// As LilyPond's, the lists are not cleared between placements, so each
/// placement adds those of the placements before it again.
pub fn get_relevant_accidentals(
    host: &mut dyn Host,
    elts: &[Value],
    left: Value,
) -> Result<Vec<Value>, Exception> {
    let (mut br, mut ra, mut ret) = (Vec::new(), Vec::new(), Vec::new());
    let right = item::break_status_dir(host.heap(), left) == 1;
    for &e in elts {
        split_accidentals(host, e, &mut br, &mut ra)?;
        ret.extend_from_slice(&ra);
        if right {
            ret.extend_from_slice(&br);
        }
    }
    Ok(ret)
}

// --- the placement (MS3 step 39) ---------------------------------------------

/// `Accidental_placement_entry`: the accidentals of one key of the
/// placement's `accidental-grobs`, and their skylines along Y.
struct Ape {
    horizontal_skylines: SkylinePair,
    grobs: Vec<Value>,
}

/// `ape_less`: the entry with more accidentals first, then the one whose
/// skylines reach less far right (`ape_priority`: right is up, the
/// skylines being horizontal).
fn ape_less(a: &Ape, b: &Ape) -> bool {
    let (size_a, size_b) = (a.grobs.len(), b.grobs.len());
    if size_a != size_b {
        return size_b < size_a;
    }
    a.horizontal_skylines.right() < b.horizontal_skylines.right()
}

/// `acc_less` of two accidentals' pitches: the lower octave first, then a
/// natural last, then the lower alteration first. `None` for a pitch
/// missing, which LilyPond takes as not less with its programming error.
fn acc_less(p: Option<&Pitch>, q: Option<&Pitch>) -> Option<bool> {
    let (p, q) = (p?, q?);
    if p.octave != q.octave {
        return Some(p.octave < q.octave);
    }
    if p.alteration.compare(Rational::ZERO) == Ordering::Equal {
        return Some(false);
    }
    if q.alteration.compare(Rational::ZERO) == Ordering::Equal {
        return Some(true);
    }
    Some(p.alteration.compare(q.alteration) == Ordering::Less)
}

/// `stagger_apes`: the entries sorted by [`ape_less`], then, among those
/// of each size, taken alternately from the back and the front, and the
/// whole reversed: an entry of more accidentals ends up closer to the
/// note column than one of fewer, octaves staying aligned.
fn stagger_apes(apes: &mut Vec<Ape>) {
    let mut order: Vec<usize> = (0..apes.len()).collect();
    lily_engine_skyline::sort(&mut order, &|&a: &usize, &b: &usize| {
        ape_less(&apes[a], &apes[b])
    });
    let mut taken: Vec<Option<Ape>> = core::mem::take(apes).into_iter().map(Some).collect();
    let mut ascs: Vec<Vec<Ape>> = Vec::new();
    let mut sz = usize::MAX;
    for i in order {
        let a = taken[i].take().expect("each entry once");
        let my_sz = a.grobs.len();
        if sz != my_sz {
            ascs.push(Vec::new());
        }
        ascs.last_mut().expect("a group").push(a);
        sz = my_sz;
    }
    for asc in ascs {
        let mut asc: Vec<Option<Ape>> = asc.into_iter().map(Some).collect();
        let (mut back, mut j) = (asc.len(), 0);
        let mut parity = true;
        while j < back {
            let a = if parity {
                back -= 1;
                asc[back].take()
            } else {
                j += 1;
                asc[j - 1].take()
            };
            apes.push(a.expect("each entry once"));
            parity = !parity;
        }
    }
    apes.reverse();
}

/// `build_apes`: an entry for each of `accs`' keys, its accidentals in the
/// list's order.
fn build_apes(heap: &Heap, accs: Value) -> Vec<Ape> {
    let mut apes = Vec::new();
    let mut entries = accs;
    while let (Some(entry), Some(rest)) = (heap.car(entries), heap.cdr(entries)) {
        let mut grobs = Vec::new();
        let mut list = heap.cdr(entry).unwrap_or(Value::NULL);
        while let (Some(g), Some(more)) = (heap.car(list), heap.cdr(list)) {
            if grob_of(heap, g).is_some() {
                grobs.push(g);
            }
            list = more;
        }
        apes.push(Ape {
            horizontal_skylines: SkylinePair::empty(),
            grobs,
        });
        entries = rest;
    }
    apes
}

/// `relative_coordinate` to `common`, the root for none.
fn coordinate(
    host: &mut dyn Host,
    g: Value,
    common: Option<Value>,
    a: Axis,
) -> Result<f64, Exception> {
    grob::relative_coordinate(host, g, common.unwrap_or(Value::NULL), a)
}

/// `translate_axis`, LilyPond's programming error for an infinite one.
fn translate(host: &mut dyn Host, g: Value, y: f64, a: Axis) -> Result<(), Exception> {
    match grob::translate_axis(host.heap(), g, y, a) {
        Ok(()) => Ok(()),
        Err(text) => grob::programming_error(host, g, text),
    }
}

/// `set_ape_skylines`: the entry's accidentals sorted by [`acc_less`] and,
/// from the last, those of one octave with another alteration than the
/// one before shifted left of it by `padding`, the same alteration printed
/// over it; and the entry's skylines, each accidental's
/// `horizontal-skylines` placed at its coordinates in `common`, merged.
fn set_ape_skylines(
    host: &mut dyn Host,
    ape: &mut Ape,
    common: [Option<Value>; 2],
    padding: f64,
) -> Result<(), Exception> {
    let mut keyed = Vec::with_capacity(ape.grobs.len());
    for (i, &a) in ape.grobs.iter().enumerate() {
        keyed.push((i, a, accidental_pitch(host, a)?));
    }
    let unpitched = Cell::new(0);
    {
        let pitches: Vec<Option<Pitch>> = keyed.iter().map(|k| k.2).collect();
        let mut order: Vec<usize> = (0..keyed.len()).collect();
        lily_engine_skyline::sort(&mut order, &|&a: &usize, &b: &usize| {
            acc_less(pitches[a].as_ref(), pitches[b].as_ref()).unwrap_or_else(|| {
                unpitched.set(unpitched.get() + 1);
                false
            })
        });
        keyed = order.into_iter().map(|i| keyed[i]).collect();
    }
    for _ in 0..unpitched.get() {
        grob::unlocated_programming_error(host, "these accidentals do not have a pitch")?;
    }
    // Accidentals of one note name, in different octaves, are taken not to
    // collide; of one octave, the same accidental is printed over the one
    // before, another one shifted left of it.
    let mut last_octave = 0;
    let mut offset = 0.0;
    let mut last_offset = 0.0;
    let mut last_alteration = Rational::ZERO;
    for i in (0..keyed.len()).rev() {
        let a = keyed[i].1;
        let Some(p) = accidental_pitch(host, a)? else {
            continue;
        };
        if i == keyed.len() - 1 || p.octave != last_octave {
            last_offset = 0.0;
            offset = grob::extent(host, a, a, X)?.left - padding;
        } else if p.alteration.compare(last_alteration) == Ordering::Equal {
            translate(host, a, last_offset, X)?;
        } else {
            let this_offset = offset - grob::extent(host, a, a, X)?.right;
            translate(host, a, this_offset, X)?;
            last_offset = this_offset;
            offset -= grob::extent(host, a, a, X)?.length() + padding;
        }
        let v = grob::property(host, a, "horizontal-skylines")?;
        let mut skyps = skylines::skyline_pair_in(host.heap(), v)
            .map_err(|message| crate::misc_error(host.heap(), "from_scm", message))?
            .unwrap_or_default();
        skyps.raise(coordinate(host, a, common[X], X)?);
        skyps.shift(coordinate(host, a, common[Y], Y)?);
        ape.horizontal_skylines.merge(&skyps);
        last_octave = p.octave;
        last_alteration = p.alteration;
    }
    Ok(())
}

/// What `uniquify` keeps of `grobs`: each once, where it first is.
fn uniquify(heap: &Heap, grobs: &mut Vec<Value>) {
    let key = |g: Value| grob_of(heap, g).map(|(arena, id)| (arena.as_ptr() as usize, id));
    let mut seen = BTreeSet::new();
    grobs.retain(|&g| key(g).is_none_or(|k| seen.insert(k)));
}

/// `extract_heads_and_stems`: the note heads of the accidentals' note
/// columns and of the note columns in the same note collisions (the heads
/// themselves where they are in no note column), then their stems, each
/// once.
fn extract_heads_and_stems(host: &mut dyn Host, apes: &[Ape]) -> Result<Vec<Value>, Exception> {
    let mut note_cols = Vec::new();
    let mut ret = Vec::new();
    for ape in apes.iter().rev() {
        for &acc in ape.grobs.iter().rev() {
            let Some(head) = grob::get_parent(host.heap(), acc, Y) else {
                continue;
            };
            match grob::get_parent(host.heap(), head, X) {
                Some(col) if grob::has_interface(host.heap(), col, "note-column-interface") => {
                    note_cols.push(col);
                }
                _ => ret.push(head),
            }
        }
    }
    // This is a little kludgy: in case there are note columns without
    // accidentals, they are taken from the note collisions.
    for i in (0..note_cols.len()).rev() {
        if let Some(c) = grob::get_parent(host.heap(), note_cols[i], X)
            && grob::has_interface(host.heap(), c, "note-collision-interface")
        {
            let columns = grob_array::extract_grob_array(host, c, "elements")?;
            note_cols.extend(columns);
        }
    }
    for i in (0..note_cols.len()).rev() {
        let note_heads = grob_array::extract_grob_array(host, note_cols[i], "note-heads")?;
        ret.extend(note_heads);
    }
    for i in (0..ret.len()).rev() {
        if let Some(s) = rhythmic_head::get_stem(host, ret[i])? {
            ret.push(s);
        }
    }
    uniquify(host.heap(), &mut ret);
    Ok(ret)
}

/// `common_refpoint_of_accidentals`: the common refpoint on `a` of all the
/// entries' accidentals, from the last.
fn common_refpoint_of_accidentals(heap: &Heap, apes: &[Ape], a: Axis) -> Option<Value> {
    let mut ret: Option<Value> = None;
    for ape in apes.iter().rev() {
        for &g in ape.grobs.iter().rev() {
            ret = match ret {
                None => Some(g),
                Some(r) => grob::common_refpoint(heap, r, g, a),
            };
        }
    }
    ret
}

/// `build_heads_skyline`: the skyline left of the heads and stems along Y,
/// from their extents in `common`, their pure ones on Y over the whole
/// score.
fn build_heads_skyline(
    host: &mut dyn Host,
    heads_and_stems: &[Value],
    common: [Option<Value>; 2],
) -> Result<Skyline, Exception> {
    let mut head_extents = Vec::with_capacity(heads_and_stems.len());
    for &g in heads_and_stems.iter().rev() {
        let x = grob::extent(host, g, common[X].unwrap_or(Value::NULL), X)?;
        let y = crate::unpure::pure_y_extent(
            host,
            g,
            common[Y].unwrap_or(Value::NULL),
            0,
            i32::MAX as usize,
        )?;
        head_extents.push(sky_box(x, y));
    }
    Ok(Skyline::from_boxes(&head_extents, Horizon::Y, Sky::Down))
}

/// `Box`, in a skyline's form.
fn sky_box(x: Interval, y: Interval) -> lily_engine_fonts::Box {
    let i = |i: Interval| lily_engine_fonts::Interval::new(i.left, i.right);
    lily_engine_fonts::Box { x: i(x), y: i(y) }
}

/// `position_apes`: the entries placed from the right, each as close to
/// the skyline of the heads (less the placement's `right-padding`) and of
/// the entries right of it as it may, `padding` apart; an entry that
/// meets nothing at the offset of the one before. The width they take.
fn position_apes(
    host: &mut dyn Host,
    me: Value,
    apes: &[Ape],
    heads_skyline: &Skyline,
) -> Result<Interval, Exception> {
    let v = grob::property(host, me, "padding")?;
    let padding = double_or(host, v, 0.2);
    let mut left_skyline = heads_skyline.clone();
    let v = grob::property(host, me, "right-padding")?;
    left_skyline.raise(-double_or(host, v, 0.0));
    // Entries added right to left.
    let mut width = Interval::EMPTY;
    let mut last_offset = 0.0;
    for ape in apes.iter().rev() {
        let mut offset = -ape.horizontal_skylines.up.distance(&left_skyline, 0.1);
        if offset.is_infinite() {
            offset = last_offset;
        } else {
            offset -= padding;
        }
        let mut new_left_skyline = ape.horizontal_skylines.down.clone();
        new_left_skyline.raise(offset);
        new_left_skyline.merge(&left_skyline);
        left_skyline = new_left_skyline;
        // All of this entry's accidentals shifted.
        for &g in ape.grobs.iter().rev() {
            translate(host, g, offset, X)?;
        }
        for sky in [&ape.horizontal_skylines.down, &ape.horizontal_skylines.up] {
            let mh = sky.max_height();
            if !mh.is_infinite() {
                width.add_point(mh + offset);
            }
        }
        last_offset = offset;
    }
    Ok(width)
}

/// `calc_positioning_done`: the accidentals of the placement `me` placed
/// left of their note columns' heads and stems and of each other, the
/// entries with more accidentals closest; `me`'s `X-extent` the width
/// they take. `#t`, and `me`'s `positioning-done` set to it first.
pub fn calc_positioning_done(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    if !grob::is_live_grob(host.heap(), me) {
        return Ok(Value::TRUE);
    }
    grob::set_property(host.heap(), me, "positioning-done", Value::TRUE);
    let sym = host.heap().intern("accidental-grobs");
    let accs = grob_array::object(host, me, sym)?;
    if host.heap().car(accs).is_none() {
        return Ok(Value::TRUE);
    }
    let mut apes = build_apes(host.heap(), accs);
    let heads_and_stems = extract_heads_and_stems(host, &apes)?;
    let heap = host.heap();
    let common_y = common_refpoint_of_accidentals(heap, &apes, Y);
    let common_y = grob::common_refpoint_of_array(heap, &heads_and_stems, common_y, Y);
    let common_x = grob::common_refpoint_of_array(heap, &heads_and_stems, Some(me), X);
    let common = [common_x, common_y];
    let v = grob::property(host, me, "padding")?;
    let padding = double_or(host, v, 0.2);
    for ape in apes.iter_mut().rev() {
        set_ape_skylines(host, ape, common, padding)?;
    }
    let heads_skyline = build_heads_skyline(host, &heads_and_stems, common)?;
    stagger_apes(&mut apes);
    let width = position_apes(host, me, &apes, &heads_skyline)?;
    grob::flush_extent_cache(host.heap(), me, X);
    let heap = host.heap();
    let (l, r) = (heap.flonum(width.left), heap.flonum(width.right));
    let width = heap.cons(l, r);
    grob::set_property(heap, me, "X-extent", width);
    Ok(Value::TRUE)
}

/// What `%host-call` runs for `ly:accidental-placement::calc-positioning-done`
/// (D127): [`calc_positioning_done`] of the grob given, with the VM as its
/// host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "accidental-placement",
    f: host_placement,
}];

fn host_placement(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    calc_positioning_done(vm, args[1])
}

/// `(ly:accidental-placement::calc-positioning-done grob)`: LilyPond's
/// subr, a primitive here so that it prints as LilyPond's; it checks its
/// grob and tail-calls `%host-call` ([`HOSTS`]).
pub(crate) fn ly_calc_positioning_done(
    cx: &mut Cx,
    args: &[Value],
) -> Result<Value, lily_engine_scheme::Thrown> {
    const SUBR: &str = "ly:accidental-placement::calc-positioning-done";
    grob::grob_arg(cx, SUBR, 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("accidental-placement");
    Ok(cx.tail_call(f, &[what, args[0]]))
}
