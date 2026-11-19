//! LilyPond's `Pitch` and `Scale` (`pitch.cc`, `scale.cc`): a pitch is an
//! octave, a step of its scale and an alteration in whole tones, and keeps
//! the scale it was made with; transposition and intervals are LilyPond's.
//!
//! What LilyPond computes in C++ `int`s is checked here: a result that does
//! not fit, or a scale with no steps, is `None`. Its rounding is done in
//! doubles, as LilyPond does it.

use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use core::cmp::Ordering;

use lily_engine_scheme::{Cx, Foreign, Heap, ImageReader, ImageWriter, Thrown, Tracer, Value};

use crate::R;
use crate::rational::Rational;

/// A scale: the tones of each step above the tonic, an octave being six.
pub struct Scale {
    pub tones: Vec<Rational>,
}

impl Scale {
    fn count(&self) -> Option<i32> {
        i32::try_from(self.tones.len()).ok().filter(|&n| n > 0)
    }

    /// `tones_at_step`: the tones of `step` in `octave` above the tonic of
    /// octave 0, `step` taken modulo the scale.
    pub fn tones_at(&self, step: i32, octave: i32) -> Option<Rational> {
        let count = self.count()?;
        let normal = step.rem_euclid(count);
        let octave = octave.checked_add(step.checked_sub(normal)? / count)?;
        let six = Rational::int(octave as i64 * 6);
        self.tones[normal as usize].checked_add(six)
    }
}

/// A pitch. `scale` is the [`Scale`] it was made with, as a Scheme value,
/// which the pitch's own methods are given.
#[derive(Clone, Copy, Debug)]
pub struct Pitch {
    pub octave: i32,
    pub notename: i32,
    pub alteration: Rational,
    pub scale: Value,
}

/// The suffix of each alteration in quarter tones, from −4.
const ACCIDENTALS: [&str; 9] = ["eses", "eseh", "es", "eh", "", "ih", "is", "isih", "isis"];

impl Pitch {
    /// `Pitch (int, int, Rational)`: in `scale`, its step taken into the
    /// scale and the octave changed to match.
    pub fn new(
        octave: i32,
        notename: i32,
        alteration: Rational,
        scale: Value,
        steps: &Scale,
    ) -> Option<Pitch> {
        let mut p = Pitch {
            octave,
            notename,
            alteration,
            scale,
        };
        p.normalize_octave(steps)?;
        Some(p)
    }

    /// `Pitch ()`: middle C's octave's C, not normalised.
    pub fn zero(scale: Value) -> Pitch {
        Pitch {
            octave: 0,
            notename: 0,
            alteration: Rational::int(0),
            scale,
        }
    }

    fn normalize_octave(&mut self, scale: &Scale) -> Option<()> {
        let count = scale.count()?;
        let normal = self.notename.rem_euclid(count);
        self.octave = self
            .octave
            .checked_add(self.notename.checked_sub(normal)? / count)?;
        self.notename = normal;
        Some(())
    }

    /// `steps`: from middle C, in steps of its scale.
    pub fn steps(&self, scale: &Scale) -> Option<i32> {
        self.octave
            .checked_mul(scale.count()?)?
            .checked_add(self.notename)
    }

    /// `tone_pitch`: from middle C, in whole tones.
    pub fn tones(&self, scale: &Scale) -> Option<Rational> {
        scale
            .tones_at(self.notename, self.octave)?
            .checked_add(self.alteration)
    }

    /// `rounded_semitone_pitch`: its tones in semitones, rounded up from a
    /// half.
    pub fn semitones(&self, scale: &Scale) -> Option<i32> {
        self.rounded(scale, 2)
    }

    /// `rounded_quartertone_pitch`.
    pub fn quartertones(&self, scale: &Scale) -> Option<i32> {
        self.rounded(scale, 4)
    }

    fn rounded(&self, scale: &Scale, parts: i64) -> Option<i32> {
        let x = self
            .tones(scale)?
            .checked_mul(Rational::int(parts))?
            .checked_add(Rational::new(1, 2))?;
        Some(libm::floor(x.to_f64()) as i32)
    }

    /// `transposed`: up by `delta`, from middle C; `scale` is this pitch's,
    /// `delta_scale` the delta's. The result keeps this pitch's scale.
    pub fn transposed(self, scale: &Scale, delta: Pitch, delta_scale: &Scale) -> Option<Pitch> {
        let mut p = self;
        let tones = p.tones(scale)?.checked_add(delta.tones(delta_scale)?)?;
        p.octave = p.octave.checked_add(delta.octave)?;
        p.notename = p.notename.checked_add(delta.notename)?;
        p.alteration = p
            .alteration
            .checked_add(tones.checked_sub(p.tones(scale)?)?)?;
        p.normalize_octave(scale)?;
        Some(p)
    }

    /// `pitch_interval`: the pitch that transposes `from` to `to`, in the
    /// default scale `default` (`Value` and steps).
    pub fn interval(
        (from, from_scale): (Pitch, &Scale),
        (to, to_scale): (Pitch, &Scale),
        (default, steps): (Value, &Scale),
    ) -> Option<Pitch> {
        let sound = to.tones(to_scale)?.checked_sub(from.tones(from_scale)?)?;
        let pt = Pitch::new(
            to.octave.checked_sub(from.octave)?,
            to.notename.checked_sub(from.notename)?,
            to.alteration.checked_sub(from.alteration)?,
            default,
            steps,
        )?;
        let rest = Pitch::new(0, 0, sound.checked_sub(pt.tones(steps)?)?, default, steps)?;
        pt.transposed(steps, rest, steps)
    }

    /// `Pitch::compare`: by octave, step, then alteration.
    pub fn compare(&self, other: &Pitch) -> Option<Ordering> {
        let by_place = (self.octave as i64 - other.octave as i64)
            .cmp(&0)
            .then((self.notename as i64 - other.notename as i64).cmp(&0));
        if by_place != Ordering::Equal {
            return Some(by_place);
        }
        let a = self.alteration.checked_sub(other.alteration)?;
        Some(a.compare(Rational::int(0)))
    }

    /// `to_string`: `c'`, `bes`, `fisis,,`; `??` for an alteration with no
    /// name.
    pub fn write(&self, scale: Option<&Scale>, out: &mut String) {
        let count = scale.and_then(Scale::count).unwrap_or(7);
        let n = (self.notename.wrapping_add(2)) % count;
        out.push(char::from((n as u8).wrapping_add(b'a')));
        let quarters = self
            .alteration
            .checked_mul(Rational::int(4))
            .unwrap_or(Rational::NAN);
        // A negative double becomes 0 as a `size_t` on arm64 (D59).
        let at = libm::rint(quarters.to_f64() + 4.0) as usize;
        out.push_str(ACCIDENTALS.get(at).copied().unwrap_or("??"));
        let (mark, marks) = if self.octave >= 0 {
            ('\'', self.octave as i64 + 1)
        } else {
            (',', -(self.octave as i64) - 1)
        };
        for _ in 0..marks {
            out.push(mark);
        }
    }
}

impl PartialEq for Pitch {
    /// `Pitch::equal_p`: the same step, octave and alteration, whatever the
    /// scales.
    fn eq(&self, other: &Pitch) -> bool {
        self.notename == other.notename
            && self.octave == other.octave
            && self.alteration == other.alteration
    }
}

// --- the Scheme face (pitch-scheme.cc, scale.cc) -------------------------------------

impl Foreign for Scale {
    fn type_name(&self) -> &'static str {
        "Scale"
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.count(self.tones.len());
        for tone in &self.tones {
            tone.save(out);
        }
        true
    }
}

impl Scale {
    pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<Scale> {
        let tones = (0..r.count()?)
            .map(|_| Rational::load(r))
            .collect::<Option<_>>()?;
        Some(Scale { tones })
    }
}

impl Foreign for Pitch {
    fn type_name(&self) -> &'static str {
        "Pitch"
    }

    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<Pitch ");
        self.write(heap.foreign::<Scale>(self.scale), out);
        out.push_str(" >");
    }

    fn equal(&self, other: &dyn Foreign, _: &Heap) -> bool {
        (other as &dyn Any).downcast_ref::<Pitch>() == Some(self)
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.scale);
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.i32(self.octave);
        out.i32(self.notename);
        self.alteration.save(out);
        out.value(self.scale);
        true
    }
}

impl Pitch {
    pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<Pitch> {
        Some(Pitch {
            octave: r.i32()?,
            notename: r.i32()?,
            alteration: Rational::load(r)?,
            scale: r.value()?,
        })
    }
}

pub(crate) fn pitch_arg(
    cx: &mut Cx,
    subr: &str,
    position: usize,
    v: Value,
) -> Result<Pitch, Thrown> {
    crate::arg(cx, subr, position, v, "Pitch")
}

/// Runs `f` on a pitch's scale, which a pitch always has.
fn with_scale<T>(cx: &Cx, p: &Pitch, f: impl FnOnce(&Scale) -> Option<T>) -> Option<T> {
    f(cx.heap().foreign::<Scale>(p.scale)?)
}

/// `default-global-scale` of `(lily)`, which a new pitch is made in.
pub(crate) fn default_scale(cx: &mut Cx, subr: &str) -> Result<Value, Thrown> {
    let scale = cx
        .global("lily", "default-global-scale")
        .unwrap_or(Value::FALSE);
    match cx.heap().foreign::<Scale>(scale) {
        Some(_) => Ok(scale),
        None => Err(cx.wrong_type_vm(Some(subr), scale, "Scale")),
    }
}

fn pitch(cx: &mut Cx, subr: &str, p: Option<Pitch>) -> R {
    match p {
        Some(p) => Ok(cx.heap_mut().wrap(p)),
        None => Err(crate::overflow(cx, subr)),
    }
}

/// `(ly:make-pitch octave note [alter])`: `note` a step of the default
/// scale, 0 for C; `alter` in whole tones.
pub(crate) fn make_pitch(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:make-pitch";
    crate::check_integer(cx, SUBR, 1, args[0])?;
    crate::check_integer(cx, SUBR, 2, args[1])?;
    let alter = match args.get(2) {
        Some(&v) => v,
        None => cx.heap_mut().integer(0),
    };
    let ok = crate::is_rational(cx.heap(), alter);
    crate::check(cx, ok, SUBR, 3, alter, "rational")?;
    let octave = crate::to_i32(cx, args[0])?;
    let note = crate::to_i32(cx, args[1])?;
    let alter = crate::to_rational(cx, alter)?;
    let scale = default_scale(cx, SUBR)?;
    let made = cx
        .heap()
        .foreign::<Scale>(scale)
        .and_then(|steps| Pitch::new(octave, note, alter, scale, steps));
    pitch(cx, SUBR, made)
}

pub(crate) fn is_pitch(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<Pitch>(cx, args[0])
}

/// `(ly:pitch-transpose p delta)`: `p` up by `delta`, which is from middle C.
pub(crate) fn pitch_transpose(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:pitch-transpose";
    let p = pitch_arg(cx, SUBR, 1, args[0])?;
    let delta = pitch_arg(cx, SUBR, 2, args[1])?;
    let heap = cx.heap();
    let made = match (
        heap.foreign::<Scale>(p.scale),
        heap.foreign::<Scale>(delta.scale),
    ) {
        (Some(scale), Some(delta_scale)) => p.transposed(scale, delta, delta_scale),
        _ => None,
    };
    pitch(cx, SUBR, made)
}

/// The interval from `from` to `to`, in the default scale.
fn interval(cx: &mut Cx, subr: &str, from: Pitch, to: Pitch) -> R {
    let default = default_scale(cx, subr)?;
    let heap = cx.heap();
    let scales = (
        heap.foreign::<Scale>(from.scale),
        heap.foreign::<Scale>(to.scale),
        heap.foreign::<Scale>(default),
    );
    let made = match scales {
        (Some(a), Some(b), Some(steps)) => Pitch::interval((from, a), (to, b), (default, steps)),
        _ => None,
    };
    pitch(cx, subr, made)
}

/// `(ly:pitch-negate p)`: the interval from `p` to middle C's octave's C.
pub(crate) fn pitch_negate(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:pitch-negate";
    let p = pitch_arg(cx, SUBR, 1, args[0])?;
    let default = default_scale(cx, SUBR)?;
    interval(cx, SUBR, p, Pitch::zero(default))
}

/// `(ly:pitch-diff pitch root)`: the delta that transposes `root` to `pitch`.
pub(crate) fn pitch_diff(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:pitch-diff";
    let p = pitch_arg(cx, SUBR, 1, args[0])?;
    let root = pitch_arg(cx, SUBR, 2, args[1])?;
    interval(cx, SUBR, root, p)
}

/// An integer of a pitch and its scale, or the overflow.
fn integer(cx: &mut Cx, subr: &str, v: Value, f: fn(&Pitch, &Scale) -> Option<i32>) -> R {
    let p = pitch_arg(cx, subr, 1, v)?;
    match with_scale(cx, &p, |scale| f(&p, scale)) {
        Some(n) => Ok(cx.heap_mut().integer(n as i64)),
        None => Err(crate::overflow(cx, subr)),
    }
}

pub(crate) fn pitch_steps(cx: &mut Cx, args: &[Value]) -> R {
    integer(cx, "ly:pitch-steps", args[0], Pitch::steps)
}

pub(crate) fn pitch_octave(cx: &mut Cx, args: &[Value]) -> R {
    integer(cx, "ly:pitch-octave", args[0], |p, _| Some(p.octave))
}

pub(crate) fn pitch_notename(cx: &mut Cx, args: &[Value]) -> R {
    integer(cx, "ly:pitch-notename", args[0], |p, _| Some(p.notename))
}

pub(crate) fn pitch_alteration(cx: &mut Cx, args: &[Value]) -> R {
    let p = pitch_arg(cx, "ly:pitch-alteration", 1, args[0])?;
    crate::rational(cx, p.alteration)
}

pub(crate) fn pitch_tones(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:pitch-tones";
    let p = pitch_arg(cx, SUBR, 1, args[0])?;
    match with_scale(cx, &p, |scale| p.tones(scale)) {
        Some(r) => crate::rational(cx, r),
        None => Err(crate::overflow(cx, SUBR)),
    }
}

pub(crate) fn pitch_quartertones(cx: &mut Cx, args: &[Value]) -> R {
    integer(cx, "ly:pitch-quartertones", args[0], Pitch::quartertones)
}

pub(crate) fn pitch_semitones(cx: &mut Cx, args: &[Value]) -> R {
    integer(cx, "ly:pitch-semitones", args[0], Pitch::semitones)
}

/// `(ly:pitch<? a b)`: by octave, step, then alteration.
pub(crate) fn pitch_less(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:pitch<?";
    let a = pitch_arg(cx, SUBR, 1, args[0])?;
    let b = pitch_arg(cx, SUBR, 2, args[1])?;
    match a.compare(&b) {
        Some(order) => Ok(Value::bool(order == Ordering::Less)),
        None => Err(crate::overflow(cx, SUBR)),
    }
}

/// `(ly:make-scale steps)`: a vector of the tones of each step above the
/// tonic.
pub(crate) fn make_scale(cx: &mut Cx, args: &[Value]) -> R {
    let steps = args[0];
    let items: Option<Vec<Value>> = cx.heap().vector_items(steps).map(<[Value]>::to_vec);
    let mut tones = Vec::new();
    let mut ok = items.is_some();
    for &step in items.iter().flatten() {
        ok = crate::is_rational(cx.heap(), step);
        if !ok {
            break;
        }
        tones.push(crate::to_rational(cx, step)?);
    }
    // SCM_ASSERT_TYPE names the C++ function.
    crate::check(cx, ok, "ly_make_scale", 1, steps, "vector of rational")?;
    Ok(cx.heap_mut().wrap(Scale { tones }))
}

pub(crate) fn is_scale(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<Scale>(cx, args[0])
}
