//! `\relative` (N2, 04 §13): `ly:make-music-relative!`, which is
//! `Music::to_relative_octave` of `music.cc`, and the five C++
//! `to-relative-callback`s that the music types name
//! (`music-sequence.cc`, `relative-octave-music.cc`,
//! `relative-octave-check.cc`), which were D67's placeholders.
//!
//! Like LilyPond's, the walk changes the music's pitches in place, so
//! frozen music is thawed as it is reached (D67). Which music it reaches
//! does not depend on the pitches, so it is known before anything changes:
//! music that holds a `to-relative-callback` of Scheme on the way (one of
//! `make-relative`'s, or a user's) is left to the fallback in [`PRELUDE`],
//! which the primitive calls in its place ([`Cx::tail_call`]), as
//! `length.rs` does. `KeyChangeEvent`'s `(lambda (x p) p)` of
//! `define-music-types.scm` is native: its type's own value is known.
//!
//! An octave check that fails warns, as LilyPond's does. The warnings are
//! given after the walk to `%music-warnings`, which issues them with
//! `ly:warning` and `ly:input-warning` (`warn.rs`) and returns the pitch.

use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{Cx, Heap, Value};

use crate::music::{At, Item};
use crate::pitch::pitch_arg;
use crate::{Music, Pitch, R, Scale, music_property, set_property};

/// The native callbacks, by their names, in [`crate::PRIMS`]' order.
pub static CALLBACKS: [&str; 5] = [
    "ly:music-sequence::event-chord-relative-callback",
    "ly:music-sequence::simultaneous-relative-callback",
    "ly:relative-octave-check::relative-callback",
    "ly:relative-octave-music::no-relative-callback",
    "ly:relative-octave-music::relative-callback",
];

/// How music is made relative.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Rule {
    /// `event_chord_relative_callback`: its elements, the first's pitch on.
    EventChord,
    /// `simultaneous_relative_callback`: its elements, the last's on.
    Simultaneous,
    /// `Relative_octave_check::relative_callback`.
    Check,
    /// The pitch given, unchanged: the two callbacks of
    /// `relative-octave-music.cc` and `KeyChangeEvent`'s lambda.
    Same,
    /// `generic_to_relative_octave`, for music without a callback.
    Generic,
}

const RULES: [Rule; 5] = [
    Rule::EventChord,
    Rule::Simultaneous,
    Rule::Check,
    Rule::Same,
    Rule::Same,
];

/// What the walk reads.
struct Keys {
    callback: Value,
    pitch: Value,
    absolute_octave: Value,
    element: Value,
    elements: Value,
    articulations: Value,
    origin: Value,
    /// The `to-relative-callback`s of `%music-properties` that are
    /// `define-music-types.scm`'s `(lambda (x p) p)`.
    same: Vec<Value>,
}

/// The text of `define-music-types.scm`'s callback that gives the pitch it
/// is given, as `build.rs` writes it.
const SAME: &str = "(lambda (x p) p)";

impl Keys {
    fn new(cx: &mut Cx) -> Keys {
        let table = cx
            .global("lily", "%music-properties")
            .unwrap_or(Value::NULL);
        let heap = cx.heap_mut();
        let callback = heap.intern("to-relative-callback");
        let mut same = Vec::new();
        for ty in crate::TYPES.iter() {
            if !ty.props.contains(&("to-relative-callback", SAME)) {
                continue;
            }
            let name = heap.intern(ty.name);
            let props = crate::music::assq(heap, name, table).and_then(|e| heap.cdr(e));
            if let Some(v) = props
                .and_then(|p| crate::music::assq(heap, callback, p))
                .and_then(|e| heap.cdr(e))
            {
                same.push(v);
            }
        }
        Keys {
            callback,
            pitch: heap.intern("pitch"),
            absolute_octave: heap.intern("absolute-octave"),
            element: heap.intern("element"),
            elements: heap.intern("elements"),
            articulations: heap.intern("articulations"),
            origin: heap.intern("origin"),
            same,
        }
    }

    /// The rule of music whose `to-relative-callback` is `callback`; None
    /// for a procedure of Scheme.
    fn rule(&self, heap: &Heap, callback: Option<Value>) -> Option<Rule> {
        let Some(callback) = callback.filter(|&c| heap.is_procedure(c)) else {
            return Some(Rule::Generic);
        };
        if self.same.contains(&callback) {
            return Some(Rule::Same);
        }
        let name = heap.primitive_name(callback)?;
        let i = CALLBACKS.iter().position(|&n| n == name)?;
        Some(RULES[i])
    }
}

// --- what the walk reaches -----------------------------------------------------------

/// Whether making `at` relative calls a procedure of Scheme.
fn reaches_scheme(heap: &Heap, k: &Keys, at: At) -> bool {
    let callback = at.property(heap, k.callback).and_then(|i| i.value());
    let list = |key| {
        at.property(heap, key).is_some_and(|i| {
            i.musics(heap)
                .into_iter()
                .flatten()
                .any(|m| reaches_scheme(heap, k, m))
        })
    };
    match k.rule(heap, callback) {
        None => true,
        Some(Rule::EventChord | Rule::Simultaneous) => list(k.elements),
        Some(Rule::Check | Rule::Same) => false,
        Some(Rule::Generic) => {
            at.property(heap, k.element)
                .and_then(|i| i.music(heap))
                .is_some_and(|m| reaches_scheme(heap, k, m))
                || list(k.articulations)
                || list(k.elements)
        }
    }
}

// --- the walk ------------------------------------------------------------------------

/// Why a walk stopped.
enum Stop {
    /// A pitch did not fit (D61).
    Overflow,
}

/// A walk: its names and the warnings it gave, each with the music's
/// `origin` or none.
struct Walk {
    k: Keys,
    warnings: Vec<(Option<Value>, String)>,
}

/// `Pitch::to_relative_octave`: `p` put in the octave that brings it
/// nearest to `last`, by steps, then up by its own octave from `c'`.
fn to_relative_octave(heap: &Heap, p: Pitch, last: Pitch) -> Option<Pitch> {
    let oct_mod = p.octave.checked_add(1)?;
    let mut up = last;
    let mut down = last;
    up.alteration = p.alteration;
    down.alteration = p.alteration;
    // `up_to` and `down_to`.
    if up.notename > p.notename {
        up.octave = up.octave.checked_add(1)?;
    }
    up.notename = p.notename;
    if down.notename < p.notename {
        down.octave = down.octave.checked_sub(1)?;
    }
    down.notename = p.notename;
    let scale = heap.foreign::<Scale>(last.scale)?;
    let h = i64::from(last.steps(scale)?);
    let distance = |q: &Pitch| Some((i64::from(q.steps(scale)?) - h).abs());
    let mut n = if distance(&up)? < distance(&down)? {
        up
    } else {
        down
    };
    n.octave = n.octave.checked_add(oct_mod)?;
    Some(n)
}

/// A pitch written as `Pitch::to_string` writes it.
fn pitch_text(heap: &Heap, p: &Pitch) -> String {
    let mut out = String::new();
    p.write(heap.foreign::<Scale>(p.scale), &mut out);
    out
}

impl Walk {
    /// `Music::to_relative_octave`.
    fn music(&mut self, heap: &mut Heap, music: Value, last: Pitch) -> Result<Pitch, Stop> {
        let callback = music_property(heap, music, self.k.callback);
        let rule = self.k.rule(heap, callback).unwrap_or(Rule::Same);
        match rule {
            Rule::EventChord => self.list(heap, music, self.k.elements, last, true),
            Rule::Simultaneous => self.list(heap, music, self.k.elements, last, false),
            Rule::Check => self.check(heap, music, last),
            Rule::Same => Ok(last),
            Rule::Generic => self.generic(heap, music, last),
        }
    }

    /// `music_list_to_relative` of `music`'s property `key`: the last
    /// pitch, or the first's if `first`.
    fn list(
        &mut self,
        heap: &mut Heap,
        music: Value,
        key: Value,
        p: Pitch,
        first: bool,
    ) -> Result<Pitch, Stop> {
        let list = music_property(heap, music, key);
        let items = music_items(heap, list);
        let (mut last, mut first_pitch) = (p, p);
        for (n, m) in items.into_iter().enumerate() {
            last = self.music(heap, m, last)?;
            if n == 0 {
                first_pitch = last;
            }
        }
        Ok(if first { first_pitch } else { last })
    }

    /// `generic_to_relative_octave`.
    fn generic(&mut self, heap: &mut Heap, music: Value, last: Pitch) -> Result<Pitch, Stop> {
        let mut last = self.own_pitch(heap, music, last)?;
        let element = music_property(heap, music, self.k.element);
        if let Some(e) = element.filter(|&e| heap.foreign::<Music>(e).is_some()) {
            last = self.music(heap, e, last)?;
        }
        self.list(heap, music, self.k.articulations, last, true)?;
        self.list(heap, music, self.k.elements, last, false)
    }

    /// `generic_to_relative_octave`'s first part: `music`'s pitch made
    /// relative to `last` and set, an `absolute-octave` that it misses
    /// warned of and taken; `last` if it has no pitch.
    fn own_pitch(&mut self, heap: &mut Heap, music: Value, last: Pitch) -> Result<Pitch, Stop> {
        let k = &self.k;
        let Some(old) =
            music_property(heap, music, k.pitch).and_then(|v| heap.foreign::<Pitch>(v).copied())
        else {
            return Ok(last);
        };
        let mut new = to_relative_octave(heap, old, last).ok_or(Stop::Overflow)?;
        let check =
            music_property(heap, music, k.absolute_octave).filter(|&v| heap.num(v).is_some());
        // `from_scm<int>`: an octave out of range is no check.
        let octave = check
            .and_then(|v| heap.to_i64(v))
            .and_then(|n| i32::try_from(n).ok());
        if let Some(octave) = octave.filter(|&o| o != new.octave) {
            let mut expected = new;
            expected.octave = octave;
            let message = alloc::format!(
                "octave check failed; expected \"{}\", found: \"{}\"",
                pitch_text(heap, &expected),
                pitch_text(heap, &new)
            );
            // `Music::warning`, at the music's origin.
            let origin = self.origin(heap, music);
            self.warnings.push((origin, message));
            new = expected;
        }
        let value = heap.wrap(new);
        set_property(heap, music, self.k.pitch, value);
        Ok(new)
    }

    /// The `origin` of `music` that a warning is given at, if it has one.
    fn origin(&self, heap: &mut Heap, music: Value) -> Option<Value> {
        music_property(heap, music, self.k.origin)
            .filter(|&o| heap.foreign::<crate::Input>(o).is_some())
    }

    /// `Relative_octave_check::relative_callback`: `last`, its octave
    /// changed to the check's if the check's pitch is not where `last`
    /// puts it, which warns at the music.
    fn check(&mut self, heap: &mut Heap, music: Value, last: Pitch) -> Result<Pitch, Stop> {
        let mut delta = 0;
        let pitch = music_property(heap, music, self.k.pitch)
            .and_then(|v| heap.foreign::<Pitch>(v).copied());
        if let Some(check) = pitch {
            let mut no_octave = check;
            no_octave.octave = -1;
            let result = to_relative_octave(heap, no_octave, last).ok_or(Stop::Overflow)?;
            if result != check {
                let message =
                    alloc::format!("Failed octave check, got: {}", pitch_text(heap, &result));
                let origin = self.origin(heap, music);
                self.warnings.push((origin, message));
                delta = check
                    .octave
                    .checked_sub(result.octave)
                    .ok_or(Stop::Overflow)?;
            }
        }
        let mut p = last;
        p.octave = p.octave.checked_add(delta).ok_or(Stop::Overflow)?;
        Ok(p)
    }
}

/// The music of a list, what is no music skipped.
fn music_items(heap: &Heap, list: Option<Value>) -> Vec<Value> {
    let mut items = Vec::new();
    let mut p = list.unwrap_or(Value::NULL);
    while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        if heap.foreign::<Music>(item).is_some() {
            items.push(item);
        }
        p = rest;
    }
    items
}

// --- the primitives ------------------------------------------------------------------

/// Makes `music` relative to `start` by `rule` (None: by its callback),
/// natively, or by `fallback` of [`PRELUDE`] if the walk would reach
/// Scheme; the last pitch, after the warnings, is the result.
fn relative(
    cx: &mut Cx,
    subr: &str,
    fallback: &str,
    music: Value,
    start: Value,
    rule: Option<Rule>,
) -> R {
    let Some(at) = At::of(cx.heap(), music) else {
        return Err(cx.wrong_type(subr, 1, music, "Music"));
    };
    let last = pitch_arg(cx, subr, 2, start)?;
    let k = Keys::new(cx);
    let heap = cx.heap();
    let scheme = match rule {
        None => reaches_scheme(heap, &k, at),
        Some(Rule::EventChord | Rule::Simultaneous) => {
            at.property(heap, k.elements).is_some_and(|i: Item| {
                i.musics(heap)
                    .into_iter()
                    .flatten()
                    .any(|m| reaches_scheme(heap, &k, m))
            })
        }
        Some(_) => false,
    };
    if scheme {
        return match cx.global("lily", fallback) {
            Some(f) => Ok(cx.tail_call(f, &[music, start])),
            None => Err(cx.error("misc-error", Some(subr), "no fallback in (lily)", &[])),
        };
    }
    let mut walk = Walk {
        k,
        warnings: Vec::new(),
    };
    let heap = cx.heap_mut();
    let result = match rule {
        None => walk.music(heap, music, last),
        Some(Rule::EventChord) => walk.list(heap, music, walk.k.elements, last, true),
        Some(Rule::Simultaneous) => walk.list(heap, music, walk.k.elements, last, false),
        Some(Rule::Check) => walk.check(heap, music, last),
        Some(Rule::Same | Rule::Generic) => Ok(last),
    };
    finish(cx, subr, walk.warnings, result)
}

/// The pitch a walk gave, after its warnings: `%music-warnings` issues
/// them and returns it.
fn finish(
    cx: &mut Cx,
    subr: &str,
    warnings: Vec<(Option<Value>, String)>,
    result: Result<Pitch, Stop>,
) -> R {
    let Ok(pitch) = result else {
        return Err(crate::overflow(cx, subr));
    };
    let heap = cx.heap_mut();
    let pitch = heap.wrap(pitch);
    if warnings.is_empty() {
        return Ok(pitch);
    }
    let mut list = Vec::with_capacity(warnings.len());
    for (origin, message) in warnings {
        let message = heap.string(message);
        list.push(heap.cons(origin.unwrap_or(Value::FALSE), message));
    }
    let list = heap.list(&list);
    match cx.global("lily", "%music-warnings") {
        Some(f) => Ok(cx.tail_call(f, &[pitch, list])),
        None => Err(cx.error(
            "misc-error",
            Some(subr),
            "no %music-warnings in (lily)",
            &[],
        )),
    }
}

/// `(ly:make-music-relative! music pitch)`: the last pitch.
pub(crate) fn make_music_relative(cx: &mut Cx, args: &[Value]) -> R {
    relative(
        cx,
        "ly:make-music-relative!",
        "%ly:make-music-relative!",
        args[0],
        args[1],
        None,
    )
}

/// The callback `CALLBACKS[i]` of `music` and `pitch`.
pub(crate) fn callback(cx: &mut Cx, i: usize, args: &[Value]) -> R {
    let fallback = alloc::format!("%{}", CALLBACKS[i]);
    relative(
        cx,
        CALLBACKS[i],
        &fallback,
        args[0],
        args[1],
        Some(RULES[i]),
    )
}

/// The fallbacks, in `(lily)`: `Music::to_relative_octave` and the
/// callbacks that walk music, for music whose walk reaches a procedure of
/// Scheme. A music's own pitch and an octave check are native
/// (`%music-relative-pitch!`, the check's callback), and so is each music
/// that reaches no Scheme.
pub(crate) const PRELUDE: &str = r#"
(define (%music-list-relative! l p first?)
  (let loop ((l l) (last p) (first p) (n 0))
    (cond ((not (pair? l)) (if first? first last))
          ((ly:music? (car l))
           (let ((last (ly:make-music-relative! (car l) last)))
             (loop (cdr l) last (if (= n 0) last first) (1+ n))))
          (else (loop (cdr l) last first n)))))
(define (%ly:make-music-relative! m p)
  (let ((callback (ly:music-property m 'to-relative-callback)))
    (if (procedure? callback)
        (callback m p)
        (let* ((last (%music-relative-pitch! m p))
               (e (ly:music-property m 'element))
               (last (if (ly:music? e) (ly:make-music-relative! e last) last)))
          (%music-list-relative! (ly:music-property m 'articulations) last #t)
          (%music-list-relative! (ly:music-property m 'elements) last #f)))))
(define (%ly:music-sequence::event-chord-relative-callback m p)
  (%music-list-relative! (ly:music-property m 'elements) p #t))
(define (%ly:music-sequence::simultaneous-relative-callback m p)
  (%music-list-relative! (ly:music-property m 'elements) p #f))
(define (%music-warnings result warnings)
  (for-each (lambda (w)
              (if (car w)
                  (ly:input-warning (car w) "~a" (cdr w))
                  (ly:warning "~a" (cdr w))))
            warnings)
  result)
"#;

/// `(%music-relative-pitch! music pitch)`: `generic_to_relative_octave`'s
/// own pitch, for the fallback: `music`'s pitch made relative to `pitch`,
/// set, and given back; `pitch` if it has none.
pub(crate) fn relative_pitch(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "%music-relative-pitch!";
    if At::of(cx.heap(), args[0]).is_none() {
        return Err(cx.wrong_type(SUBR, 1, args[0], "Music"));
    }
    let last = pitch_arg(cx, SUBR, 2, args[1])?;
    let mut walk = Walk {
        k: Keys::new(cx),
        warnings: Vec::new(),
    };
    let result = walk.own_pitch(cx.heap_mut(), args[0], last);
    finish(cx, SUBR, walk.warnings, result)
}
