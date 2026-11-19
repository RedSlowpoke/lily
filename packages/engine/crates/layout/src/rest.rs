//! `rest.cc` (part 3's, MS3): `ly:rest::width` (`rest.cc:281–289`, 5
//! lines from `SCM`) and what it calls, `generic_extent_callback`
//! (`:301–314`, 14), `brew_internal_stencil` (`:230–257`, 28) and
//! `glyph_name` (`:166–227`, 62) without its ledgers, ported first (MS3
//! step 16): `Dot_column::add_head` takes a dotted rest's width
//! (`dot-column.cc:255`), and T26 has one. A breve's rest calls
//! `Staff_symbol_referencer::staff_space` (`%staff-space`,
//! `staff_symbol_referencer.rs` since MS3 step 27).
//!
//! The rest of the file (MS3 step 37): `glyph_name`'s ledgers
//! (`:170–185`, 16) with `Staff_symbol_referencer::on_staff_line`
//! (`staff-symbol-referencer.cc:33–38`, 6) and the staff lines of
//! `Staff_symbol::on_line` (`staff-symbol.cc:372–381`, 10), which
//! `on_staff_line` asks without the ledgers; `ly:rest::print`
//! (`:275–279`, 5), `::height` (`:291–296`, 6), `::pure-height`
//! (`:316–323`, 8), `::y-offset-callback` (`:33–45`, 13) over
//! `staff_position_internal` (`:47–145`, 99) and `::calc-cross-staff`
//! (`:150–161`, 12), the music crate's placeholders until then (D67).
//! `Rest::translate` (`:263–273`) is the rest collision's, MS3 step 41
//! (`rest_collision.rs`).
//!
//! The callbacks are the image's Scheme, as layout's others are.

use alloc::string::String;

use lily_engine_scheme::{Cx, Value};

use crate::grob;

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(%grob-warning grob text)`: `Grob::warning`, at the grob's origin.
pub(crate) fn grob_warning(cx: &mut Cx, args: &[Value]) -> R {
    let mut text = String::new();
    cx.heap().display(args[1], &mut text);
    grob::reported(cx, args[0], "warning", Value::UNSPECIFIED, &[text])
}

/// The callbacks, and what they compute with:
///
/// - `%scm->int`, `%scm->double` and `%double-or`: `scm_to_int`,
///   `scm_to_double` and `from_scm<double> (v, fallback)` as Guile
///   converts.
/// - `%staff-on-staff-line?`, `Staff_symbol_referencer::on_staff_line`:
///   whether the position `pos` is on one of the staff symbol's lines,
///   `#f` without a staff symbol; the ledgers are not asked.
/// - `%rest-glyph-name`, `glyph_name`: with `try-ledgers`, a half, whole
///   or breve rest off the staff lines (a breve neither on one nor
///   hanging from one) at its position plus `offset`, cut to an `int`,
///   is the ledgered glyph, `o`; never of the mensural styles.
/// - `%rest-stencil`, `brew_internal_stencil`: the glyph, ledgered or
///   not, a breve's moved up by a staff space less its font size's scale.
/// - `%rest-staff-position`, `staff_position_internal`: the position the
///   rest's `Y-offset` gives it, in half staff spaces; rest-collision.cc
///   asks it too (MS3 step 41).
pub(crate) const PRELUDE: &str = r#"
(define (%scm->int v)
  (cond ((not (and (integer? v) (exact? v)))
         (scm-error 'wrong-type-arg #f "Wrong type (expecting ~A): ~S"
                    (list "exact integer" v) (list v)))
        ((not (<= -2147483648 v 2147483647))
         (scm-error 'out-of-range #f "Value out of range ~S to< ~S: ~S"
                    (list -2147483648 2147483647 v) (list v)))
        (else v)))
(define (%scm->double v)
  (if (real? v)
      (exact->inexact v)
      (scm-error 'wrong-type-arg #f "Wrong type (expecting ~A): ~S"
                 (list "real number" v) (list v))))
(define (%double-or v fallback) (if (number? v) (%scm->double v) fallback))
(define (%staff-on-staff-line? me pos)
  (let ((st (%staff-symbol me)))
    (and st
         (let loop ((lines (%staff-symbol-line-positions st)))
           (cond ((null? lines) #f)
                 ((= pos (car lines)) #t)
                 (else (loop (cdr lines))))))))
(define (%rest-glyph-name me durlog style try-ledgers offset)
  (let* ((ledgered
          (and try-ledgers (<= -1 durlog 1)
               (let ((pos (inexact->exact (truncate (+ (%staff-position me #f) offset)))))
                 (and (not (%staff-on-staff-line? me pos))
                      (not (and (= durlog -1) (%staff-on-staff-line? me (+ pos 2))))))))
         (mensural (member style '("mensural" "neomensural")))
         (ledgered (and ledgered (not mensural)))
         (actual (if (and mensural (> durlog 4)) "" style))
         (actual (if (and (member style '("classical" "z")) (not (= durlog 2))) "" actual))
         (actual (if (string=? style "default") "" actual)))
    (string-append "rests." (number->string durlog) (if ledgered "o" "") actual)))
(define (%rest-stencil grob ledgered)
  (let ((durlog (ly:grob-property grob 'duration-log)))
    (if (not (number? durlog))
        (ly:make-stencil '())
        (let* ((durlog (%scm->int durlog))
               (style (let ((s (ly:grob-property grob 'style)))
                        (if (symbol? s) (symbol->string s) "default")))
               (font (ly:grob-default-font grob))
               (name (%rest-glyph-name grob durlog style ledgered 0.0))
               (out (ly:font-get-glyph font name)))
          (if (ly:stencil-empty? out)
              (%grob-warning grob (string-append "rest `" name "' not found")))
          (if (< durlog 0)
              (let ((fs (expt 2 (/ (%double-or (ly:grob-property grob 'font-size) 0.0) 6)))
                    (ss (%staff-space grob)))
                (ly:stencil-translate-axis out (- ss fs) 1))
              out)))))
(define (%rest-staff-position me durlog dir)
  (let ((sp (ly:grob-property me 'staff-position)))
    (if (number? sp)
        (let ((pos (%real-or sp 0.0)))
          (if (= durlog 0) (+ pos 2) pos))
        (let* ((vpos (* dir (%int-or (ly:grob-property me 'voiced-position) 0)))
               (pos (exact->inexact vpos))
               (staff (and (<= durlog 1) (%staff-symbol me)))
               (linepos (if staff (%staff-symbol-line-positions staff) '())))
          (cond
           ((null? linepos) pos)
           ((and (null? (cdr linepos)) (< durlog 0)
                 (eqv? 0 (%direction-or (ly:grob-property me 'direction) 0)))
            (- (car linepos) 2))
           (else
            (let* ((sorted (sort linepos <))
                   (pos (if (= durlog 0)
                            ;; upper_bound, or the last line
                            (let ((pos (if (< dir 0) (- pos 2) pos)))
                              (let loop ((ls sorted))
                                (cond ((> (car ls) pos) (car ls))
                                      ((null? (cdr ls)) (car ls))
                                      (else (loop (cdr ls))))))
                            ;; the line before upper_bound, or the first
                            (let loop ((ls (cdr sorted)) (prev (car sorted)))
                              (if (or (null? ls) (> (car ls) pos))
                                  prev
                                  (loop (cdr ls) (car ls)))))))
              (if (eqv? dir 0)
                  pos
                  (let ((neutral (%rest-staff-position me durlog 0)))
                    (if (> (* dir (- pos neutral)) 0)
                        pos
                        (+ neutral vpos)))))))))))
(define-public (ly:rest::y-offset-callback grob)
  (%check-type "ly:rest::y-offset-callback" 1 ly:grob? "Grob" grob)
  (let* ((durlog (%scm->int (ly:grob-property grob 'duration-log)))
         (ss (%staff-space grob))
         (dir (%direction-or (ly:grob-property grob 'direction) 0)))
    (* (* ss 0.5) (%rest-staff-position grob durlog dir))))
(define-public (ly:rest::calc-cross-staff grob)
  (%check-type "ly:rest::calc-cross-staff" 1 ly:grob? "Grob" grob)
  (let ((stem (ly:grob-object grob 'stem)))
    (if (ly:grob? stem) (ly:grob-property stem 'cross-staff) #f)))
(define-public (ly:rest::print grob)
  (%check-type "ly:rest::print" 1 ly:grob? "Grob" grob)
  (%rest-stencil grob #t))
(define-public (ly:rest::width grob)
  (%check-type "ly:rest::width" 1 ly:grob? "Grob" grob)
  (ly:stencil-extent (%rest-stencil grob #f) 0))
(define-public (ly:rest::height grob)
  (%check-type "ly:rest::height" 1 ly:grob? "Grob" grob)
  (ly:stencil-extent (%rest-stencil grob #t) 1))
(define-public (ly:rest::pure-height grob start end)
  (%check-type "ly:rest::pure-height" 1 ly:grob? "Grob" grob)
  (ly:stencil-extent (%rest-stencil grob #f) 1))
"#;
