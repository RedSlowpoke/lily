//! Transposition of music (N2, 04 §13): `ly:music-transpose`, which is
//! `Prob::transpose` of `music.cc`, and `ly:transpose-key-alist` of
//! `music-scheme.cc`, which it calls for a key's `pitch-alist`.
//!
//! LilyPond changes the music in place, and so does the engine: frozen
//! music is thawed as the walk reaches it (D67), and the parser freezes it
//! again when the function that called this returns.

use alloc::vec::Vec;

use lily_engine_scheme::{Cx, Heap, Value};

use crate::dispatcher::StreamEvent;
use crate::music::{At, Item, deep_copy, is_true_property};
use crate::pitch::{default_scale, pitch_arg};
use crate::{Music, Pitch, R, Rational, Scale, mutable_properties};

/// The property names the walk reads.
struct Keys {
    untransposable: Value,
    tonic: Value,
    element: Value,
    elements: Value,
    articulations: Value,
    pitch_alist: Value,
}

impl Keys {
    fn new(heap: &mut Heap) -> Keys {
        Keys {
            untransposable: heap.intern("untransposable"),
            tonic: heap.intern("tonic"),
            element: heap.intern("element"),
            elements: heap.intern("elements"),
            articulations: heap.intern("articulations"),
            pitch_alist: heap.intern("pitch-alist"),
        }
    }
}

/// `p` transposed by `delta` (`Pitch::transposed`), or None if it did not
/// fit (D61).
fn transposed(heap: &Heap, p: Pitch, delta: Pitch) -> Option<Pitch> {
    let scale = heap.foreign::<Scale>(p.scale)?;
    let delta_scale = heap.foreign::<Scale>(delta.scale)?;
    p.transposed(scale, delta, delta_scale)
}

/// `Prob::transpose`: each pitch among `music`'s mutable properties
/// transposed by `delta` (a `tonic` to octave -1), its `element`, its
/// `elements` and `articulations`, and its `pitch-alist`; nothing of music
/// that is `untransposable`. None if a pitch did not fit.
fn transpose(heap: &mut Heap, k: &Keys, music: Value, delta: Pitch, scale: Value) -> Option<()> {
    if is_true_property(heap, music, k.untransposable) {
        return Some(());
    }
    let entries = mutable_properties(heap, music);
    transpose_alist(heap, k, entries, delta, scale)
}

/// `Prob::transpose`'s walk over a mutable property list.
fn transpose_alist(
    heap: &mut Heap,
    k: &Keys,
    mut entries: Value,
    delta: Pitch,
    scale: Value,
) -> Option<()> {
    while let (Some(entry), Some(rest)) = (heap.car(entries), heap.cdr(entries)) {
        entries = rest;
        let (Some(key), Some(val)) = (heap.car(entry), heap.cdr(entry)) else {
            continue;
        };
        if let Some(&p) = heap.foreign::<Pitch>(val) {
            let mut t = transposed(heap, p, delta)?;
            if key == k.tonic {
                let steps = heap.foreign::<Scale>(t.scale)?;
                t = Pitch::new(-1, t.notename, t.alteration, t.scale, steps)?;
            }
            let new = heap.wrap(t);
            heap.set_cdr(entry, new);
        } else if key == k.element {
            if heap.foreign::<Music>(val).is_some() {
                transpose(heap, k, val, delta, scale)?;
            }
        } else if key == k.elements || key == k.articulations {
            for m in music_items(heap, val) {
                transpose(heap, k, m, delta, scale)?;
            }
        } else if key == k.pitch_alist && heap.car(val).is_some() {
            let new = transpose_key_alist(heap, val, delta, scale)?;
            heap.set_cdr(entry, new);
        }
    }
    Some(())
}

/// `Stream_event::make_transposable`, then `Prob::transpose`, of the stream
/// event `ev`, a clone (`Quote_iterator`): its music's pitches, `element`,
/// `elements` and `pitch-alist` that its own properties lack copied into
/// them (`music_deep_copy`), and its own properties transposed by `delta`.
/// None if a pitch did not fit.
pub(crate) fn transpose_event(
    heap: &mut Heap,
    ev: Value,
    delta: Pitch,
    scale: Value,
) -> Option<()> {
    let k = Keys::new(heap);
    let music = heap.foreign::<StreamEvent>(ev)?.music;
    if let Some(at) = At::of(heap, music) {
        for (key, item) in at.entries(heap) {
            let Item::Value(val) = item else {
                continue;
            };
            let wanted = heap.foreign::<Pitch>(val).is_some()
                || (key == k.element && heap.foreign::<Music>(val).is_some())
                || ((key == k.elements || key == k.pitch_alist) && heap.car(val).is_some());
            let props = heap.foreign::<StreamEvent>(ev)?.props;
            if !wanted || has_key(heap, props, key) {
                continue;
            }
            let copy = deep_copy(heap, val);
            let entry = heap.cons(key, copy);
            let props = heap.cons(entry, props);
            heap.foreign_mut::<StreamEvent>(ev)?.props = props;
        }
    }
    let untransposable = heap
        .foreign::<StreamEvent>(ev)?
        .property(heap, k.untransposable);
    if untransposable.is_some_and(|v| v != Value::FALSE) {
        return Some(());
    }
    // The music a frozen node holds is in the event's own properties as
    // handles its source event shares (`music_event`): copied, as
    // `make_transposable` copies it.
    let props = heap.foreign::<StreamEvent>(ev)?.props;
    let mut p = props;
    while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        p = rest;
        let (Some(key), Some(val)) = (heap.car(entry), heap.cdr(entry)) else {
            continue;
        };
        if key == k.element || key == k.elements {
            let copy = deep_copy(heap, val);
            heap.set_cdr(entry, copy);
        }
    }
    transpose_alist(heap, &k, props, delta, scale)
}

/// Whether the alist `props` has `key`.
fn has_key(heap: &Heap, mut props: Value, key: Value) -> bool {
    while let (Some(entry), Some(rest)) = (heap.car(props), heap.cdr(props)) {
        if heap.car(entry) == Some(key) {
            return true;
        }
        props = rest;
    }
    false
}

/// `transpose_music_list`'s `as_ly_smob_list<Prob>`: the music of a list,
/// what is no music skipped.
fn music_items(heap: &Heap, list: Value) -> Vec<Value> {
    let mut items = Vec::new();
    let mut p = list;
    while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        if heap.foreign::<Music>(item).is_some() {
            items.push(item);
        }
        p = rest;
    }
    items
}

/// `ly_transpose_key_alist`: a new key alist of `alist`'s entries
/// `((octave . step) . alter)` and `(step . alter)` transposed by `delta`,
/// in the default scale `scale`; other entries are dropped. None if a
/// pitch did not fit, or an entry's numbers are none.
fn transpose_key_alist(heap: &mut Heap, alist: Value, delta: Pitch, scale: Value) -> Option<Value> {
    let mut out = Vec::new();
    let mut p = alist;
    while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        p = rest;
        let (key, alter) = (heap.car(entry)?, heap.cdr(entry)?);
        let alter = rational(heap, alter)?;
        let steps = heap.foreign::<Scale>(scale)?;
        if let (Some(octave), Some(step)) = (heap.car(key), heap.cdr(key)) {
            let (octave, step) = (int(heap, octave)?, int(heap, step)?);
            let orig = Pitch::new(octave, step, alter, scale, steps)?;
            let t = transposed(heap, orig, delta)?;
            let (o, n) = (
                heap.integer(t.octave.into()),
                heap.integer(t.notename.into()),
            );
            let key = heap.cons(o, n);
            let alter = rational_value(heap, t.alteration)?;
            out.push(heap.cons(key, alter));
        } else if heap.num(key).is_some() {
            let orig = Pitch::new(0, int(heap, key)?, alter, scale, steps)?;
            let t = transposed(heap, orig, delta)?;
            let n = heap.integer(t.notename.into());
            let alter = rational_value(heap, t.alteration)?;
            out.push(heap.cons(n, alter));
        }
    }
    Some(heap.list(&out))
}

fn int(heap: &Heap, v: Value) -> Option<i32> {
    i32::try_from(heap.to_i64(v)?).ok()
}

/// `from_scm<Rational>` of an exact number.
fn rational(heap: &Heap, v: Value) -> Option<Rational> {
    match heap.as_ratio(v) {
        Some((n, d)) => Some(Rational::new(heap.to_i64(n)?, heap.to_i64(d)?)),
        None => Some(Rational::int(heap.to_i64(v)?)),
    }
}

/// `to_scm (Rational)` of a finite rational.
fn rational_value(heap: &mut Heap, r: Rational) -> Option<Value> {
    let n = heap.integer(r.numerator());
    let d = heap.integer(r.denominator());
    heap.div(n, d).ok()
}

/// `(ly:music-transpose m p)`: `m` transposed in place so that middle C
/// goes to `p`; `m` is returned.
pub(crate) fn music_transpose(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:music-transpose";
    if At::of(cx.heap(), args[0]).is_none() {
        return Err(cx.wrong_type(SUBR, 1, args[0], "Music"));
    }
    let delta = pitch_arg(cx, SUBR, 2, args[1])?;
    let scale = default_scale(cx, SUBR)?;
    let k = Keys::new(cx.heap_mut());
    match transpose(cx.heap_mut(), &k, args[0], delta, scale) {
        Some(()) => Ok(args[0]),
        None => Err(crate::overflow(cx, SUBR)),
    }
}

/// `(ly:transpose-key-alist l pit)`.
pub(crate) fn transpose_key_alist_prim(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:transpose-key-alist";
    let delta = pitch_arg(cx, SUBR, 2, args[1])?;
    let scale = default_scale(cx, SUBR)?;
    match transpose_key_alist(cx.heap_mut(), args[0], delta, scale) {
        Some(v) => Ok(v),
        None => Err(crate::overflow(cx, SUBR)),
    }
}
