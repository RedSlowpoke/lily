//! `staff-symbol.cc` (part 3's, MS3): the staff symbol's callbacks
//! `ly:staff-symbol::calc-line-positions` (`staff-symbol.cc:105–118`, 14
//! lines), `::height` (`:340–363`, 24) and `::print` (`:33–103`, 71), with
//! what they call of the file, `staff_space` (`:321–327`, 7),
//! `get_line_thickness`'s computation (`:329–335`, 7, inline in the
//! callbacks as LilyPond has it) and `line_span` (`:391–411`, 21), MS3 step
//! 27. They were the music crate's placeholders (D67). The ledgers'
//! positions (`ledger_positions`, `:121–319`, 199 lines) and `on_line`
//! (`:371–396`, 26), which the dot column asks of every dot, MS3 step 37,
//! ported first from the ledger line spanner's step (39), Rust over
//! `%host-call` (D127, [`HOSTS`]); `get_ledger_line_thickness` is with
//! its one caller, the ledger line spanner's print (`ledger_line_spanner.rs`,
//! MS3 step 39).
//!
//! The callbacks are the image's Scheme, as layout's others are: they read
//! properties through their callbacks, which a primitive cannot call. The
//! stencil is made by `%staff-symbol-lines` ([`lines`]), which adds the
//! lines as `Stencil::add_stencil` does, where `ly:stencil-add` would make
//! another expression. `::print` takes a bound's edge from
//! `Paper_column::break_align_width` (`paper_column.rs`).
//!
//! Where LilyPond's C++ would crash, the doc comments of [`PRELUDE`] say
//! what the port does instead.

use alloc::vec::Vec;

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::geometry::{Interval, Y};
use crate::grob;
use crate::lookup;
use crate::stencil::{self, Stencil};

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(%staff-symbol-lines span thickness positions space)`: `::print`'s
/// stencil, a `Lookup::horizontal_line` over `span`, a pair of reals,
/// `thickness` thick, added once for each of the reals `positions`,
/// translated by half `space` a position. Its arguments are what
/// [`PRELUDE`] has converted.
pub(crate) fn lines(cx: &mut Cx, args: &[Value]) -> R {
    let heap = cx.heap_mut();
    let real = |v: Option<Value>| v.and_then(|v| heap.to_f64(v)).unwrap_or(0.0);
    let span = Interval::new(real(heap.car(args[0])), real(heap.cdr(args[0])));
    let (t, space) = (real(Some(args[1])), real(Some(args[3])));
    let mut positions = alloc::vec::Vec::new();
    let mut p = args[2];
    while let (Some(x), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        positions.push(real(Some(x)));
        p = rest;
    }
    let line = lookup::horizontal_line(heap, span, t);
    let mut m = Stencil::default();
    for p in positions {
        let mut b = line;
        b.translate_axis(heap, p * 0.5 * space, Y);
        m.add_stencil(heap, &b);
    }
    stencil::wrap(cx, m)
}

/// `from_scm_list<std::vector<Real>>`: each real of the pairs of `l`,
/// Guile's type error at the first that is none.
fn reals(host: &mut dyn Host, mut l: Value) -> Result<Vec<f64>, Exception> {
    let mut out = Vec::new();
    while let (Some(x), Some(rest)) = (host.heap().car(l), host.heap().cdr(l)) {
        out.push(grob::to_double(host.heap(), x)?);
        l = rest;
    }
    Ok(out)
}

/// `ledger_positions`: the positions of the ledger lines a head at `pos`
/// needs on the staff symbol `me`: the head's `ledger-positions`, if a
/// list; else what `me`'s `ledger-positions-function`, an expression,
/// evaluates to called with `me` and `pos`, if a procedure; else from the
/// staff line nearest `pos` (the one nearer the middle line at a tie) to
/// `pos` and `ledger-extra` beyond it (the head's, unless `'()`, else
/// `me`'s), `me`'s `ledger-positions` repeated by its cycle (a group
/// whole or not at all) or every other position, none on a staff line;
/// none for no lines, or for a head on a line with neither.
pub fn ledger_positions(
    host: &mut dyn Host,
    me: Value,
    pos: i32,
    head: Option<Value>,
) -> Result<Vec<f64>, Exception> {
    if let Some(head) = head {
        let posns = grob::property(host, head, "ledger-positions")?;
        if host.heap().car(posns).is_some() {
            return reals(host, posns);
        }
    }
    let lp_function = grob::property(host, me, "ledger-positions-function")?;
    if host.heap().car(lp_function).is_some() {
        let env = crate::call_lily(host, "interaction-environment", &[])?;
        let func = crate::call_lily(host, "eval", &[lp_function, env])?;
        if host.heap().is_procedure(func) {
            let p = host.heap().integer(i64::from(pos));
            let v = host.call(func, &[me, p])?;
            return reals(host, v);
        }
    }
    let ledger_positions = grob::property(host, me, "ledger-positions")?;
    let mut extra = Value::NULL;
    if let Some(head) = head {
        extra = grob::property(host, head, "ledger-extra")?;
    }
    if extra == Value::NULL {
        extra = grob::property(host, me, "ledger-extra")?;
    }
    let ledger_extra = if host.heap().is_number(extra) {
        grob::to_double(host.heap(), extra)?
    } else {
        0.0
    };
    let lines = crate::call_lily(host, "%staff-symbol-line-positions", &[me])?;
    let line_positions = reals(host, lines)?;
    let mut values = Vec::new();
    if line_positions.is_empty() {
        return Ok(values);
    }
    let pos_f = f64::from(pos);
    // The staff line nearest the position, the inner one at a tie.
    let mut nearest_line = 0.0;
    let mut line_dist = f64::INFINITY;
    for &p in &line_positions {
        let mut dist = (p - pos_f).abs();
        if (p >= 0.0 && pos > 0 && p < pos_f) || (p <= 0.0 && pos < 0 && p > pos_f) {
            dist = libm::nextafter(dist, 0.0);
        }
        if dist < line_dist {
            nearest_line = p;
            line_dist = dist;
        }
    }
    let custom = host.heap().car(ledger_positions).is_some();
    if line_dist == 0.0 && !custom && ledger_extra == 0.0 {
        return Ok(values);
    }
    let extra_dir = if pos_f - nearest_line > 0.0 {
        1.0
    } else if pos_f - nearest_line < 0.0 || nearest_line <= 0.0 {
        -1.0
    } else {
        1.0
    };
    let mut extra_pos = pos_f + ledger_extra * extra_dir;
    if (extra_pos - nearest_line) * extra_dir < 0.0 {
        extra_pos = nearest_line;
    }
    let mut fill = Interval::EMPTY;
    fill.add_point(nearest_line);
    fill.add_point(extra_pos);
    let contains = |v: f64| fill.left <= v && v <= fill.right;
    if custom {
        // The pattern's extent, its groups' too.
        let (mut min_pos, mut max_pos) = (f64::INFINITY, f64::NEG_INFINITY);
        let mut pattern = Vec::new();
        let mut s = ledger_positions;
        while let (Some(s2), Some(rest)) = (host.heap().car(s), host.heap().cdr(s)) {
            let group = if host.heap().car(s2).is_some() {
                Some(reals(host, s2)?)
            } else {
                None
            };
            for &l in group.as_deref().unwrap_or(&[]) {
                max_pos = if l > max_pos { l } else { max_pos };
                min_pos = if l < min_pos { l } else { min_pos };
            }
            if group.is_none() {
                let l = grob::to_double(host.heap(), s2)?;
                max_pos = if l > max_pos { l } else { max_pos };
                min_pos = if l < min_pos { l } else { min_pos };
            }
            pattern.push((s2, group));
            s = rest;
        }
        let cycle = max_pos - min_pos;
        // From a multiple of the cycle at or below the fill's bottom.
        let n = (fill.left - min_pos) / cycle;
        let multiple = libm::floor(n) as i32;
        let mut offset = f64::from(multiple) * cycle;
        if cycle < 0.1 {
            return Ok(values);
        }
        let mut i = 0;
        loop {
            match &pattern[i] {
                (s2, None) => {
                    let current = grob::to_double(host.heap(), *s2)? + offset;
                    if contains(current) {
                        values.push(current);
                    }
                }
                (_, Some(group)) => {
                    // A group, all of it or none.
                    let values2: Vec<f64> = group.iter().map(|v| v + offset).collect();
                    if values2.iter().any(|&v| contains(v)) {
                        values.extend_from_slice(&values2);
                    }
                }
            }
            i += 1;
            if i == pattern.len() {
                i = 0;
                offset += cycle;
            }
            // `while (offset + min_pos <= fill[UP])`, NaN ending it too.
            if (offset + min_pos)
                .partial_cmp(&fill.right)
                .is_none_or(|o| o.is_gt())
            {
                break;
            }
        }
    } else {
        // Every other position from the nearest line.
        let bottom = libm::ceil(fill.left) as i32;
        let top = libm::floor(fill.right) as i32;
        let nearest = if nearest_line < 0.0 {
            libm::floor(nearest_line)
        } else {
            libm::ceil(nearest_line)
        };
        let odd_start = (nearest as i32) & 1;
        let skew = i32::from((bottom & 1) != odd_start);
        let num_ledgers = (top - bottom + 2 - skew) / 2;
        let mut value = bottom + skew;
        for _ in 0..num_ledgers.max(0) {
            values.push(f64::from(value));
            value += 2;
        }
    }
    // None on a staff line.
    values.retain(|v| !line_positions.contains(v));
    Ok(values)
}

/// `on_line`: whether `pos` is on one of the staff symbol `me`'s lines,
/// or, with `allow_ledger`, on one of the ledger lines a head there needs
/// ([`ledger_positions`]).
pub fn on_line(
    host: &mut dyn Host,
    me: Value,
    pos: i32,
    allow_ledger: bool,
) -> Result<bool, Exception> {
    let lines = crate::call_lily(host, "%staff-symbol-line-positions", &[me])?;
    let pos = f64::from(pos);
    if reals(host, lines)?.contains(&pos) {
        return Ok(true);
    }
    Ok(allow_ledger && ledger_positions(host, me, pos as i32, None)?.contains(&pos))
}

/// What `%host-call` runs for `note_head.rs`' `%staff-on-line?` (D127):
/// [`on_line`] of the staff symbol and position given, ledgers allowed,
/// with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "staff-symbol-on-line",
    f: host_on_line,
}];

fn host_on_line(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let pos = crate::stem::to_int(vm.heap_mut(), args[2])?;
    Ok(if on_line(vm, args[1], pos, true)? {
        Value::TRUE
    } else {
        Value::FALSE
    })
}

/// The three callbacks, and what they compute with:
///
/// - `%real-or`, `from_scm<double> (v, fallback)`: a real, else the
///   fallback; `%layout-dimension`, `Output_def::get_dimension` of the
///   grob's layout, Guile's type error for no real.
/// - `%staff-symbol-staff-space`, `Staff_symbol::staff_space`, and
///   `%staff-symbol-line-thickness`, the line thickness of `::print` and
///   `::height` (`get_line_thickness`): the layout's dimension times the
///   grob's `staff-space` or `thickness`, 1 unset.
/// - `%staff-symbol-line-positions`, `from_scm_list<std::vector<Real>>`
///   of `line-positions`: each real of its pairs, Guile's type error at the
///   first that is none; `%staff-symbol-line-span`, `line_span`: their
///   interval, `(1 . -1)` for none.
///
/// `::calc-line-positions` makes no positions of a negative `line-count`,
/// where LilyPond's `std::vector` would throw. `::print` of a staff
/// symbol without both bounds is the type error of
/// `ly:grob-common-refpoint`, where LilyPond dereferences no item.
pub(crate) const PRELUDE: &str = r#"
(define (%real-or v fallback) (if (real? v) (exact->inexact v) fallback))
(define (%layout-dimension grob sym)
  (%scm->double (ly:output-def-lookup (ly:grob-layout grob) sym)))
(define (%staff-symbol-staff-space me)
  (let ((ss (%layout-dimension me 'staff-space)))
    (* (%real-or (ly:grob-property me 'staff-space) 1.0) ss)))
(define (%staff-symbol-line-thickness me)
  (let ((t (%layout-dimension me 'line-thickness)))
    (* t (%real-or (ly:grob-property me 'thickness) 1.0))))
(define (%staff-symbol-line-positions me)
  (let loop ((s (ly:grob-property me 'line-positions)) (out '()))
    (if (pair? s)
        (loop (cdr s) (cons (%scm->double (car s)) out))
        (reverse! out))))
(define (%staff-symbol-line-span me)
  (let ((ps (%staff-symbol-line-positions me)))
    (if (null? ps)
        (cons 1.0 -1.0)
        (let loop ((ps ps) (lo +inf.0) (hi -inf.0))
          (if (null? ps)
              (cons lo hi)
              (loop (cdr ps) (min lo (car ps)) (max hi (car ps))))))))
(define-public (ly:staff-symbol::calc-line-positions grob)
  (%check-type "ly:staff-symbol::calc-line-positions" 1 ly:grob? "Grob" grob)
  (let* ((v (ly:grob-property grob 'line-count))
         (count (if (and (integer? v) (exact? v) (<= -2147483648 v 2147483647)) v 0))
         (height (exact->inexact (- count 1))))
    (let loop ((i (- count 1)) (out '()))
      (if (< i 0)
          out
          (loop (- i 1) (cons (- height (* i 2)) out))))))
(define-public (ly:staff-symbol::height grob)
  (%check-type "ly:staff-symbol::height" 1 ly:grob? "Grob" grob)
  (let ((ext (%staff-symbol-line-span grob)))
    (if (> (car ext) (cdr ext))
        (cons 0.0 0.0)
        (let* ((f (* 0.5 (%staff-symbol-staff-space grob)))
               (lo (* (car ext) f))
               (hi (* (cdr ext) f))
               (ext (if (< f 0) (cons hi lo) (cons lo hi)))
               (t (%staff-symbol-line-thickness grob)))
          (cons (- (car ext) (/ t 2)) (+ (cdr ext) (/ t 2)))))))
(define (%staff-symbol-span-point me x common d t)
  (let* ((width (ly:grob-property me 'width))
         (p (cond
             ((and (eqv? d 1) (number? width)) (%scm->double width))
             ((or (let ((e (%grob-extent x x 0))) (> (car e) (cdr e)))
                  (and (not (eqv? 0 (ly:item-break-dir x)))
                       (ly:grob? (ly:spanner-broken-neighbor me d))))
              (%grob-relative-coordinate x common 0))
             (else
              (let* ((where (if (eqv? d 1)
                                (ly:grob-property me 'break-align-symbols)
                                'break-alignment))
                     (w (%break-align-width x where)))
                (if (eqv? d 1) (cdr w) (car w)))))))
    (- p (/ (* d t) 2))))
(define-public (ly:staff-symbol::print grob)
  (%check-type "ly:staff-symbol::print" 1 ly:spanner? "Spanner" grob)
  (let* ((left (ly:spanner-bound grob -1))
         (right (ly:spanner-bound grob 1))
         (common (ly:grob-common-refpoint left right 0))
         (t (%staff-symbol-line-thickness grob))
         (l (%staff-symbol-span-point grob left common -1 t))
         (r (%staff-symbol-span-point grob right common 1 t))
         (positions (%staff-symbol-line-positions grob))
         (me-x (%grob-relative-coordinate grob common 0))
         (space (%staff-symbol-staff-space grob)))
    (%staff-symbol-lines (cons (- l me-x) (- r me-x)) t positions space)))
"#;
