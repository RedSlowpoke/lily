//! `self-alignment-interface.cc` (part 3's, MS3 step 27): a grob aligned on
//! its own extent and on its X parent's. The five callbacks the grobs
//! name, the music crate's placeholders until then (D67):
//! `ly:self-alignment-interface::y-aligned-on-self`, `::x-aligned-on-self`
//! and `::pure-y-aligned-on-self` (`self-alignment-interface.cc:29–54`, 26
//! lines) over `aligned_on_self` (`:56–70`, 15), and `::centered-on-x-parent`
//! (`:78–85`, 8) over `centered_on_self` (`:72–76`, 5) and
//! `::aligned-on-x-parent` (`:96–103`, 8) over `aligned_on_parent`
//! (`:114–169`, 56); 118 lines. `aligned_on_parent` asks part 3's
//! `Paper_column::get_interface_extent` (`paper-column.cc:263–276`, 14,
//! `paper_column.rs`), ported first.
//!
//! The Y-parent forms, `::centered-on-y-parent` and `::aligned-on-y-parent`
//! (`:87–94`, `:105–112`), and `set_aligned_on_parent` (`:171–177`) are
//! names no grob of the engine's gives and no ported code calls; their
//! computations are the same functions on the Y axis
//! (`%centered-on-self`, `%aligned-on-parent`).
//!
//! The callbacks are the image's Scheme, as layout's others are: they read
//! properties and extents through their callbacks. Two primitives compute
//! what Scheme cannot as LilyPond does: [`linear_combination`], with
//! `Interval::linear_combination`'s one rounding (`geometry.rs`), and
//! [`has_interface`].

use alloc::string::String;

use lily_engine_scheme::{Cx, Value};

use crate::geometry::Interval;
use crate::grob;

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(%interval-linear-combination ext x)`: `ext.linear_combination (x)`,
/// of a pair of reals and a real that [`PRELUDE`] has converted.
pub(crate) fn linear_combination(cx: &mut Cx, args: &[Value]) -> R {
    let heap = cx.heap_mut();
    let real = |v: Option<Value>| v.and_then(|v| heap.to_f64(v)).unwrap_or(0.0);
    let ext = Interval::new(real(heap.car(args[0])), real(heap.cdr(args[0])));
    let x = real(Some(args[1]));
    Ok(cx.heap_mut().flonum(ext.linear_combination(x)))
}

/// `(%grob-has-interface? grob sym)`: `internal_has_interface`, whether
/// `grob` has the interface named `sym` ([`grob::has_interface`]); `#f`
/// for what is no grob or no symbol.
pub(crate) fn has_interface(cx: &mut Cx, args: &[Value]) -> R {
    let Some(name) = cx.heap().name(args[1]).map(String::from) else {
        return Ok(Value::FALSE);
    };
    Ok(Value::bool(grob::has_interface(
        cx.heap_mut(),
        args[0],
        &name,
    )))
}

/// The five callbacks, and what they compute with:
///
/// - `%self-aligned me a pure start end`, `aligned_on_self`: minus the
///   `self-alignment-X` or `-Y` combination of the grob's own extent on
///   `a`, its pure extent between the columns `start` and `end` on Y when
///   `pure`; 0 for no number or an empty extent.
/// - `%vsize-or`, `from_scm (v, vsize {fallback})`: an exact integer from
///   0 to 2⁶⁴ − 1, else the fallback, 0 for `start` and `INT_MAX` for `end`.
/// - `%centered-on-self`, `centered_on_self`: the center of the grob's
///   robust extent relative to itself.
/// - `%aligned-on-parent`, `aligned_on_parent`: the `parent-alignment-X`
///   or `-Y` combination (the self-alignment's for `'()`) of the parent's
///   extent, less the `self-alignment-X` or `-Y` combination of its own:
///   of a paper column, the extent of its note columns, else its
///   `X-alignment-extent` on X; of a note column with the grob's
///   `X-align-on-main-noteheads` `#t`, its `main-extent`.
///
/// A grob without the parent `::centered-on-x-parent` or
/// `::aligned-on-x-parent` asks for is Guile's type error, where LilyPond
/// dereferences no grob.
pub(crate) const PRELUDE: &str = r#"
(define (%interval-empty? i) (> (car i) (cdr i)))
(define (%scm->interval v) (cons (%scm->double (car v)) (%scm->double (cdr v))))
(define (%vsize-or v fallback)
  (if (and (integer? v) (exact? v) (<= 0 v 18446744073709551615)) v fallback))
(define (%self-aligned me a pure start end)
  (let ((align (ly:grob-property me (if (eqv? a 0) 'self-alignment-X 'self-alignment-Y))))
    (if (number? align)
        (let ((ext (if (and pure (eqv? a 1))
                       (%grob-pure-y-extent me me start end)
                       (%grob-extent me me a))))
          (if (%interval-empty? ext)
              0.0
              (- (%interval-linear-combination ext (%scm->double align)))))
        0.0)))
(define (%centered-on-self me a)
  (let ((ext (%grob-robust-relative-extent me me a)))
    (/ (+ (car ext) (cdr ext)) 2.0)))
(define (%aligned-on-parent me a)
  (let* ((him (ly:grob-parent me a))
         (he (cond ((%grob-has-interface? him 'paper-column-interface)
                    (let ((he (%paper-column-interface-extent him 'note-column-interface a)))
                      (if (and (%interval-empty? he) (eqv? a 0))
                          (let ((ext (ly:grob-property him 'X-alignment-extent)))
                            (if (%number-pair? ext) (%scm->interval ext) he))
                          he)))
                   ((and (eq? (ly:grob-property me 'X-align-on-main-noteheads) #t)
                         (%grob-has-interface? him 'note-column-interface))
                    (%scm->interval (ly:grob-property him 'main-extent)))
                   (else (%grob-extent him him a))))
         (self-align (ly:grob-property me (if (eqv? a 0) 'self-alignment-X 'self-alignment-Y)))
         (par-align (let ((p (ly:grob-property
                              me (if (eqv? a 0) 'parent-alignment-X 'parent-alignment-Y))))
                      (if (null? p) self-align p)))
         (ext (%grob-extent me me a))
         (x (if (and (number? self-align) (not (%interval-empty? ext)))
                (- 0.0 (%interval-linear-combination ext (%scm->double self-align)))
                0.0)))
    (if (and (number? par-align) (not (%interval-empty? he)))
        (+ x (%interval-linear-combination he (%scm->double par-align)))
        x)))
(define-public (ly:self-alignment-interface::y-aligned-on-self element)
  (%check-type "ly:self-alignment-interface::y-aligned-on-self" 1 ly:grob? "Grob" element)
  (%self-aligned element 1 #f 0 0))
(define-public (ly:self-alignment-interface::x-aligned-on-self element)
  (%check-type "ly:self-alignment-interface::x-aligned-on-self" 1 ly:grob? "Grob" element)
  (%self-aligned element 0 #f 0 0))
(define-public (ly:self-alignment-interface::pure-y-aligned-on-self smob start end)
  (%check-type "ly:self-alignment-interface::pure-y-aligned-on-self" 1 ly:grob? "Grob" smob)
  (%self-aligned smob 1 #t (%vsize-or start 0) (%vsize-or end 2147483647)))
(define-public (ly:self-alignment-interface::centered-on-x-parent smob)
  (%check-type "ly:self-alignment-interface::centered-on-x-parent" 1 ly:grob? "Grob" smob)
  (%centered-on-self (ly:grob-parent smob 0) 0))
(define-public (ly:self-alignment-interface::aligned-on-x-parent smob)
  (%check-type "ly:self-alignment-interface::aligned-on-x-parent" 1 ly:grob? "Grob" smob)
  (%aligned-on-parent smob 0))
"#;
