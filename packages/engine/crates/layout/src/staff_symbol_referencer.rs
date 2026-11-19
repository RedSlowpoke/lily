//! `staff-symbol-referencer.cc` and `staff-symbol-referencer-scheme.cc`
//! (part 3's, MS3 step 27): a grob placed by its staff position against
//! its staff symbol. `ly:staff-symbol-referencer::callback`
//! (`staff-symbol-referencer.cc:123–139`, 17 lines), the music crate's
//! placeholder until then (D67), with what is read through the
//! referencer: `get_staff_symbol` and `staff_space` (`:40–53`, 14, rest.rs'
//! until then), `line_thickness` (`:55–62`, 8), the position, pure and not
//! (`:64–94`, 31), the extent in the staff (`:96–109`, 14), the rounded
//! positions (`:111–121`, 11), `set_position` (`:153–177`, 25),
//! `staff_span` and `staff_radius` (`:179–196`, 18) and the orders by
//! position (`:198–217`, 20); and the Scheme face's
//! `ly:grob-staff-position`, `ly:staff-symbol-line-thickness`,
//! `ly:staff-symbol-staff-space` and `ly:staff-symbol-staff-radius`
//! (`staff-symbol-referencer-scheme.cc:29–41` and `:56–90`, 48).
//!
//! `on_line` and `on_staff_line` (`:26–38`) ask `Staff_symbol::on_line`
//! (`staff_symbol.rs`, MS3 step 37): they are `note_head.rs`' and
//! `rest.rs`' `%staff-on-line?` and `%staff-on-staff-line?`.
//! `ly:position-on-line?` (`-scheme.cc:43–54`), which came with the
//! custodes (MS8 step 9, part 2: `custos::print` asks it), is
//! `%staff-on-line?` of the position, an exact integer as `from_scm<int>`
//! takes it.
//!
//! Since MS3 step 46 (05 §5.1's machinery) Rust over a [`Host`], the
//! callback in the callback table too (`callbacks.rs`). [`PRELUDE`]'s
//! callback, `%staff-space` and `%staff-position` call the Rust over
//! `%host-call` ([`HOSTS`], D127), and keep the image's Scheme of each for
//! a run left waiting for the host, where a `%host-call` would be
//! cancelled (as a traced compile's reads are after a part's file is asked
//! for, T22); the rest is the Scheme, for its Scheme callers. The staff symbol's own functions are `staff_symbol.rs`'
//! Scheme, `%staff-symbol-staff-space`, `%staff-symbol-line-thickness`
//! and `%staff-symbol-line-span`, called through the image.

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::geometry::{Interval, Y};
use crate::grob;

/// A real that `staff_symbol.rs`' Scheme `name` gives of the staff symbol
/// `staff`.
fn staff_real(host: &mut dyn Host, name: &str, staff: Value) -> Result<f64, Exception> {
    let v = crate::call_lily(host, name, &[staff])?;
    Ok(host.heap().to_f64(v).unwrap_or(0.0))
}

/// `get_staff_symbol`: the grob's `staff-symbol` object if it is a grob.
pub(crate) fn staff_symbol(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    let sym = host.heap().intern("staff-symbol");
    let st = grob::object(host, me, sym)?;
    Ok(grob_of(host.heap(), st).map(|_| st))
}

/// `staff_space`: the staff symbol's, else 1.
pub(crate) fn staff_space(host: &mut dyn Host, me: Value) -> Result<f64, Exception> {
    match staff_symbol(host, me)? {
        Some(st) => staff_real(host, "%staff-symbol-staff-space", st),
        None => Ok(1.0),
    }
}

/// `line_thickness`: the staff symbol's, else the layout's
/// `line-thickness`.
pub(crate) fn line_thickness(host: &mut dyn Host, me: Value) -> Result<f64, Exception> {
    match staff_symbol(host, me)? {
        Some(st) => staff_real(host, "%staff-symbol-line-thickness", st),
        None => {
            let sym = host.heap().intern("line-thickness");
            let v = crate::call_lily(host, "%layout-dimension", &[me, sym])?;
            Ok(host.heap().to_f64(v).unwrap_or(0.0))
        }
    }
}

/// The staff symbol, if the grob has one, and its common refpoint on Y
/// with the grob, if they have one.
fn staff_and_common(
    host: &mut dyn Host,
    me: Value,
) -> Result<Option<(Value, Option<Value>)>, Exception> {
    let st = staff_symbol(host, me)?;
    Ok(st.map(|st| (st, grob::common_refpoint(host.heap(), me, st, Y))))
}

/// `internal_get_position`: with a staff symbol in a common refpoint,
/// twice the Y distance from it in staff spaces (0 for a staff space of
/// 0), the pure coordinate from rank 0 to `INT_MAX`; with none, twice its
/// offset from its Y parent (its absolute Y without one); with a staff
/// symbol but no common refpoint, its `staff-position`, 0 for none.
pub(crate) fn get_position(host: &mut dyn Host, me: Value, pure: bool) -> Result<f64, Exception> {
    match staff_and_common(host, me)? {
        Some((st, Some(c))) => {
            let y = if pure {
                let end = Value::fixnum(i64::from(i32::MAX)).unwrap_or(Value::NULL);
                let args = [me, c, Value::fixnum(0).unwrap_or(Value::NULL), end];
                let v = crate::call_lily(host, "%grob-pure-relative-y-coordinate", &args)?;
                host.heap().to_f64(v).unwrap_or(0.0)
            } else {
                grob::relative_coordinate(host, me, c, Y)?
            };
            let y = y - grob::relative_coordinate(host, st, c, Y)?;
            let space = staff_real(host, "%staff-symbol-staff-space", st)?;
            Ok(if space == 0.0 { 0.0 } else { 2.0 * y / space })
        }
        None => {
            let parent = grob::get_parent(host.heap(), me, Y).unwrap_or(Value::NULL);
            Ok(grob::relative_coordinate(host, me, parent, Y)? * 2.0)
        }
        Some((_, None)) => {
            let v = grob::property(host, me, "staff-position")?;
            let heap = host.heap();
            Ok(if heap.is_real(v) {
                heap.to_f64(v).unwrap_or(0.0)
            } else {
                0.0
            })
        }
    }
}

/// `get_rounded_position`: `rint` of [`get_position`], an exact integer;
/// Guile's error for an infinite one, or NaN.
pub(crate) fn rounded_position(
    host: &mut dyn Host,
    me: Value,
    pure: bool,
) -> Result<Value, Exception> {
    let r = libm::rint(get_position(host, me, pure)?);
    if r.is_finite() && libm::fabs(r) < 9.0e15 {
        return Ok(host.heap().integer(r as i64));
    }
    let r = host.heap().flonum(r);
    crate::call_lily(host, "inexact->exact", &[r])
}

/// `internal_set_position`: the grob moved by the positions from where it
/// is to `p`, LilyPond's programming error for an infinite distance.
pub(crate) fn set_position(
    host: &mut dyn Host,
    me: Value,
    p: f64,
    pure: bool,
) -> Result<(), Exception> {
    let oldpos = match staff_and_common(host, me)? {
        Some((_, Some(_))) => get_position(host, me, pure)?,
        _ => 0.0,
    };
    let ss = staff_space(host, me)?;
    if let Err(text) = grob::translate_axis(host.heap(), me, (p - oldpos) * ss * 0.5, Y) {
        grob::programming_error(host, me, text)?;
    }
    Ok(())
}

/// `on_line` (`:26–32`): whether `pos` is on a line of the grob's staff
/// symbol or a ledger line there; false without one.
pub(crate) fn on_line(host: &mut dyn Host, me: Value, pos: i32) -> Result<bool, Exception> {
    match staff_symbol(host, me)? {
        Some(st) => crate::staff_symbol::on_line(host, st, pos, true),
        None => Ok(false),
    }
}

/// `staff_span`: the staff symbol's `line_span`, empty without one.
pub(crate) fn staff_span(host: &mut dyn Host, me: Value) -> Result<Interval, Exception> {
    let Some(st) = staff_symbol(host, me)? else {
        return Ok(Interval::EMPTY);
    };
    let span = crate::call_lily(host, "%staff-symbol-line-span", &[st])?;
    let heap = host.heap();
    let end = |e: Option<Value>, or: f64| e.and_then(|e| heap.to_f64(e)).unwrap_or(or);
    Ok(Interval::new(
        end(heap.car(span), f64::INFINITY),
        end(heap.cdr(span), f64::NEG_INFINITY),
    ))
}

/// `extent_in_staff`: the grob's Y extent from its staff symbol, empty
/// without one in a common refpoint.
pub(crate) fn extent_in_staff(host: &mut dyn Host, me: Value) -> Result<Interval, Exception> {
    match staff_and_common(host, me)? {
        Some((st, Some(c))) => {
            let mut e = grob::extent(host, me, c, Y)?;
            let y = grob::relative_coordinate(host, st, c, Y)?;
            e.left -= y;
            e.right -= y;
            Ok(e)
        }
        _ => Ok(Interval::EMPTY),
    }
}

/// `staff_radius`: a quarter of [`staff_span`]'s length, 0 for an empty
/// span.
pub(crate) fn staff_radius(host: &mut dyn Host, me: Value) -> Result<f64, Exception> {
    let span = staff_span(host, me)?;
    let length = if span.left > span.right {
        0.0
    } else {
        span.right - span.left
    };
    Ok(length / 4.0)
}

/// `ly:staff-symbol-referencer::callback` (`:123–139`): half the grob's
/// `staff-position` in staff spaces, 0 for none.
fn callback(host: &mut dyn Host, grob: Value) -> Result<Value, Exception> {
    let pos = grob::property(host, grob, "staff-position")?;
    let offset = if host.heap().is_number(pos) {
        let space = staff_space(host, grob)?;
        grob::to_double(host.heap(), pos)? * space / 2.0
    } else {
        0.0
    };
    Ok(host.heap().flonum(offset))
}

/// The callbacks the callback table finds (`callbacks.rs`).
pub(crate) const CALLBACKS: [(&str, Callback); 1] = [("callback", callback)];

/// What `%host-call` runs for [`PRELUDE`]'s `(%host-call
/// 'staff-symbol-referencer what grob …)` (D127): the callback, the staff
/// space and the position in Rust, with the VM as their host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "staff-symbol-referencer",
    f: host_referencer,
}];

fn host_referencer(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let x = match vm.heap().name(args[1]) {
        Some("callback") => return callback(vm, args[2]),
        Some("staff-space") => staff_space(vm, args[2])?,
        Some("position") => get_position(vm, args[2], args[3] != Value::FALSE)?,
        _ => {
            let none = "no such staff symbol referencer call";
            return Err(crate::misc_error(vm.heap_mut(), "%host-call", none));
        }
    };
    Ok(vm.heap_mut().flonum(x))
}

/// The referencer's functions, under `%`-names where LilyPond has no
/// Scheme name:
///
/// - `%staff-symbol`, `get_staff_symbol`: the grob's `staff-symbol`
///   object if it is a grob, else `#f`. `%staff-space` and
///   `%staff-line-thickness`, `staff_space` and `line_thickness`: the staff
///   symbol's, else 1 and the layout's `line-thickness`.
/// - `%staff-position grob pure`, `internal_get_position`: with a staff
///   symbol in a common refpoint, twice the Y distance from it in staff
///   spaces (0 for a staff space of 0), the pure coordinate from rank 0
///   to `INT_MAX`; with none, twice its offset from its Y parent (its
///   absolute Y without one); with a staff symbol but no common refpoint,
///   its `staff-position`, 0 for none. `%rounded-staff-position`, `rint`
///   of it.
/// - `%staff-extent`, `extent_in_staff`: its Y extent from the staff
///   symbol, empty without one in a common refpoint.
/// - `%set-staff-position! grob p pure`, `internal_set_position`: moves
///   it by the positions from where it is to `p`.
/// - `%staff-span`, `staff_span`: the staff symbol's `line_span`, empty
///   without one; `%staff-radius`, `staff_radius`: a quarter of its
///   length.
/// - `%staff-position-compare`, `%staff-position<?` and
///   `%pure-staff-position<?`: `compare_position`, `position_less` and
///   `pure_position_less`.
///
/// `ly:grob-staff-position` gives an exact integer for a position within
/// 10⁻⁶ of one (`round_halfway_up`, `floor (x + 0.5)`), the real else;
/// `ly:position-on-line?` is `%staff-on-line?`.
pub(crate) const PRELUDE: &str = r#"
(define (%staff-symbol me)
  (let ((st (ly:grob-object me 'staff-symbol)))
    (and (ly:grob? st) st)))
(define (%staff-space me)
  (if (%host-waiting?)
      (let ((st (%staff-symbol me)))
        (if st (%staff-symbol-staff-space st) 1.0))
      (%host-call 'staff-symbol-referencer 'staff-space me)))
(define (%staff-line-thickness me)
  (let ((st (%staff-symbol me)))
    (if st
        (%staff-symbol-line-thickness st)
        (%layout-dimension me 'line-thickness))))
(define (%staff-position me pure)
  (if (%host-waiting?)
      (%staff-scheme-position me pure)
      (%host-call 'staff-symbol-referencer 'position me pure)))
(define (%staff-scheme-position me pure)
  (let* ((st (%staff-symbol me))
         (c (and st (ly:grob-common-refpoint me st 1))))
    (cond ((and st c)
           (let* ((y (- (if pure
                            (%grob-pure-relative-y-coordinate me c 0 2147483647)
                            (%grob-relative-coordinate me c 1))
                        (%grob-relative-coordinate st c 1)))
                  (space (%staff-symbol-staff-space st)))
             (if (= space 0) 0.0 (/ (* 2.0 y) space))))
          ((not st) (* (%grob-relative-coordinate me (ly:grob-parent me 1) 1) 2))
          (else (%real-or (ly:grob-property me 'staff-position) 0.0)))))
(define (%staff-extent me)
  (let* ((st (%staff-symbol me))
         (c (and st (ly:grob-common-refpoint me st 1))))
    (if (and st c)
        (let ((e (%grob-extent me c 1))
              (y (%grob-relative-coordinate st c 1)))
          (cons (- (car e) y) (- (cdr e) y)))
        (cons +inf.0 -inf.0))))
(define (%rounded-staff-position me pure)
  (inexact->exact (round (%staff-position me pure))))
(define-public (ly:staff-symbol-referencer::callback grob)
  (%check-type "ly:staff-symbol-referencer::callback" 1 ly:grob? "Grob" grob)
  (if (%host-waiting?)
      (let ((pos (ly:grob-property grob 'staff-position)))
        (if (number? pos)
            (let ((space (%staff-space grob)))
              (/ (* (%scm->double pos) space) 2.0))
            0.0))
      (%host-call 'staff-symbol-referencer 'callback grob)))
(define (%set-staff-position! me p pure)
  (let* ((st (%staff-symbol me))
         (oldpos (if (and st (ly:grob-common-refpoint me st 1))
                     (%staff-position me pure)
                     0.0))
         (ss (%staff-space me)))
    (ly:grob-translate-axis! me (* (- p oldpos) ss 0.5) 1)))
(define (%staff-span me)
  (let ((symb (%staff-symbol me)))
    (if symb (%staff-symbol-line-span symb) (cons +inf.0 -inf.0))))
(define (%staff-radius me)
  (let ((span (%staff-span me)))
    (/ (if (> (car span) (cdr span)) 0.0 (- (cdr span) (car span))) 4.0)))
(define (%staff-position-compare a b)
  (let ((d (- (%staff-position a #f) (%staff-position b #f))))
    (cond ((> d 0) 1) ((< d 0) -1) (else 0))))
(define (%staff-position<? a b)
  (< (%staff-position a #f) (%staff-position b #f)))
(define (%pure-staff-position<? a b)
  (< (%staff-position a #t) (%staff-position b #t)))
(define-public (ly:grob-staff-position sg)
  (%check-type "ly:grob-staff-position" 1 ly:grob? "Grob" sg)
  (let ((pos (%staff-position sg #f)))
    (if (< (abs (- (round pos) pos)) 1e-6)
        (inexact->exact (floor (+ pos 0.5)))
        pos)))
(define-public (ly:position-on-line? sg spos)
  (%check-type "ly:position-on-line?" 1 ly:grob? "Grob" sg)
  (%check-type "ly:position-on-line?" 2 number? "number" spos)
  (%staff-on-line? sg (%scm->int spos)))
(define-public (ly:staff-symbol-line-thickness grob)
  (%check-type "ly:staff-symbol-line-thickness" 1 ly:grob? "Grob" grob)
  (%staff-line-thickness grob))
(define-public (ly:staff-symbol-staff-space grob)
  (%check-type "ly:staff-symbol-staff-space" 1 ly:grob? "Grob" grob)
  (%staff-space grob))
(define-public (ly:staff-symbol-staff-radius grob)
  (%check-type "ly:staff-symbol-staff-radius" 1 ly:grob? "Grob" grob)
  (%staff-radius grob))
"#;
