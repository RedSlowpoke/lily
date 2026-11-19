//! `page-layout-problem.cc`'s spacing specs (part 5's, ported first by MS3
//! step 28, since the alignment and the axis groups' staff spacing call
//! them): [`read_spacing_spec`] (`page-layout-problem.cc:1185–1195`, 11
//! lines), [`get_fixed_spacing`] (`:1201–1245`, 45), `add_stretchability`
//! (`:1247–1255`, 9, step 34's in part 3's order, which
//! `get_spacing_spec` calls) and [`get_spacing_spec`] (`:1266–1342`, 77).
//!
//! A spacing spec is an alist of `basic-distance`, `minimum-distance`,
//! `padding` and `stretchability`, as `staff-staff-spacing` and the
//! `nonstaff-…-spacing` properties give it. None has a Scheme name.
//! [`get_spacing_spec`] and [`get_fixed_spacing`] are [`PRELUDE`]'s, since
//! the alignment and the page-layout problem, which are Scheme, call them
//! (MS3 step 28); Rust reaches them through a [`Host`].
//!
//! The pure left bound of `get_fixed_spacing` (`System::get_pure_bound`)
//! is the Paper_score's, by its break ranks (`%system-pure-bound`,
//! `paper_score.rs`, MS3 step 32).

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, Heap, Value};

use crate::grob::{assq_ref, to_double};

/// `LARGE_STRETCH`: the stretch between a loose line and the staff it has
/// no affinity to.
pub const LARGE_STRETCH: f64 = 10e5;
/// `HUGE_STRETCH`: the stretch of a spring with no spacing wishes, as to
/// the top or bottom of the page.
pub const HUGE_STRETCH: f64 = 10e7;

/// `read_spacing_spec`: the number `sym` of the spacing spec `spec`, if
/// its first entry of `sym` (`scm_sloppy_assq`, which skips what is no
/// pair) has a number; Guile's error for a number that is no real.
/// [`PRELUDE`]'s `%read-spacing-spec` is the same in Scheme.
pub fn read_spacing_spec(
    heap: &mut Heap,
    spec: Value,
    sym: &str,
) -> Result<Option<f64>, Exception> {
    let sym = heap.intern(sym);
    match assq_ref(heap, sym, spec) {
        Some(v) if heap.is_number(v) => to_double(heap, v).map(Some),
        _ => Ok(None),
    }
}

/// `get_fixed_spacing`: the spacing forced between `before` and `after`,
/// both staves (`is_spaceable`), `after` the (`spaceable_index` + 1)th
/// staff of its alignment: the item `spaceable_index` − 1 of the
/// `alignment-distances` in the `line-break-system-details` of the left
/// bound of `before`'s system, if it is a number; else −∞. With `pure`, the
/// system is the root of `before`'s X parents, and the result is kept as
/// `after`'s `spaceable-fixed-spacing` (`loose-fixed-spacing` for a loose
/// line) from `start`, whatever `end`. [`PRELUDE`]'s `%fixed-spacing`.
pub fn get_fixed_spacing(
    host: &mut dyn Host,
    before: Value,
    after: Value,
    spaceable_index: i64,
    pure: bool,
    start: usize,
    end: usize,
) -> Result<f64, Exception> {
    let f = crate::lily_procedure(host, "%fixed-spacing")?;
    let heap = host.heap();
    let index = heap.integer(spaceable_index);
    let (start, end) = (heap.integer(start as i64), heap.integer(end as i64));
    let v = host.call(f, &[before, after, index, Value::bool(pure), start, end])?;
    Ok(host.heap().to_f64(v).unwrap_or(0.0))
}

/// `get_spacing_spec`: the spacing spec of the spring from `before` to
/// `after`, each a staff or a loose line (`is_spaceable`), their
/// properties read pure between `start` and `end` with `pure`:
///
/// - none of either: no wishes, a stretchability of [`HUGE_STRETCH`];
/// - two staves: `before`'s `staff-staff-spacing`;
/// - a staff and a loose line: the line's `nonstaff-relatedstaff-spacing`,
///   or, when its `staff-affinity` turns away from the staff (`DOWN` below
///   it, `UP` above), its `nonstaff-unrelatedstaff-spacing` with a
///   stretchability of [`LARGE_STRETCH`] unless it has one;
/// - two loose lines: `before`'s `nonstaff-nonstaff-spacing`, or its
///   `nonstaff-unrelatedstaff-spacing` with [`LARGE_STRETCH`] when it is
///   `UP` and `after` `DOWN`. The warning "staff-affinities should only
///   decrease", once, not pure, when `after`'s affinity is greater.
///
/// [`PRELUDE`]'s `%spacing-spec`.
pub fn get_spacing_spec(
    host: &mut dyn Host,
    before: Option<Value>,
    after: Option<Value>,
    pure: bool,
    start: usize,
    end: usize,
) -> Result<Value, Exception> {
    let f = crate::lily_procedure(host, "%spacing-spec")?;
    let heap = host.heap();
    let (start, end) = (heap.integer(start as i64), heap.integer(end as i64));
    let (before, after) = (
        before.unwrap_or(Value::FALSE),
        after.unwrap_or(Value::FALSE),
    );
    host.call(f, &[before, after, Value::bool(pure), start, end])
}

/// The spacing specs in Scheme:
///
/// - `%read-spacing-spec spec sym`: [`read_spacing_spec`], `#f` for none.
/// - `%add-stretchability alist stretch`: `alist` with `stretch` as its
///   `stretchability` in front, unless it has one (`scm_sloppy_assq`).
/// - `%assq key alist`, Guile's `scm_assq`: the entry of `key`, `#f` for
///   none, with its error for an item that is no pair or an improper end
///   (checked against LilyPond 2.26.0: `(ly:assoc-get 'a '(1))` is
///   `assq`'s "association list" error, position 2).
/// - `%assoc-get key alist default`, `ly_assoc_get`: `scm_assoc`, which for
///   a symbol is `assq`.
/// - `%robust-list-ref i l`: the item `i` of the pair `l`, or its last
///   for a list too short (and its first for `i` below 0).
/// - `%system? g`: whether `g` is a `System` (`dynamic_cast`).
/// - `%root-system g`, the static `Grob::get_system (Grob *)`: the root of
///   `g`'s X parents if it is a system, else `#f`.
/// - `%system-maybe-pure-bound sys d pure start end`,
///   `System::get_maybe_pure_bound`: the system's bound at `d`, `#f` for
///   none; pure, the column of the rank `start` (`LEFT`) or `end` among
///   the break ranks (`get_pure_bound`, `%system-pure-bound`).
/// - `%fixed-spacing before after index pure start end`:
///   [`get_fixed_spacing`]. The left bound is read only for two staves, as
///   it counts for nothing else, so that a pure call between other lines
///   does not reach `get_pure_bound`; where `after` is no spanner,
///   LilyPond's pure call would crash, and nothing is kept for it here.
/// - `%spacing-spec before after pure start end`: [`get_spacing_spec`],
///   `before` or `after` none for what is no grob. The `staff-affinity` is
///   `from_scm<Direction>`: an exact −1, 0 or 1, else 0. LilyPond warns
///   once a process (a `static bool`); once a VM here.
pub(crate) const PRELUDE: &str = r#"
(define (%sloppy-assq sym l)
  (let loop ((p l))
    (cond ((not (pair? p)) #f)
          ((and (pair? (car p)) (eq? (caar p) sym)) (car p))
          (else (loop (cdr p))))))
(define (%read-spacing-spec spec sym)
  (let ((entry (%sloppy-assq sym spec)))
    (and entry (number? (cdr entry)) (%scm->double (cdr entry)))))
(define (%add-stretchability alist stretch)
  (if (%sloppy-assq 'stretchability alist)
      alist
      (cons (cons 'stretchability stretch) alist)))
(define (%assq key alist)
  (define (malformed)
    (scm-error 'wrong-type-arg "assq" "Wrong type argument in position ~A (expecting ~A): ~S"
               (list 2 "association list" alist) (list alist)))
  (let loop ((p alist))
    (cond ((pair? p)
           (cond ((not (pair? (car p))) (malformed))
                 ((eq? (caar p) key) (car p))
                 (else (loop (cdr p)))))
          ((null? p) #f)
          (else (malformed)))))
(define (%assoc-get key alist default)
  (let ((entry (%assq key alist)))
    (if entry (cdr entry) default)))
(define (%robust-list-ref i l)
  (if (and (> i 0) (pair? (cdr l)))
      (%robust-list-ref (1- i) (cdr l))
      (car l)))
(define (%root-system g)
  (let ((p (ly:grob-parent g 0)))
    (cond ((ly:grob? p) (%root-system p))
          ((%system? g) g)
          (else #f))))
(define (%system-maybe-pure-bound sys d pure start end)
  (if pure
      (%system-pure-bound sys d start end)
      (let ((b (ly:spanner-bound sys d)))
        (and (ly:grob? b) b))))
(define (%fixed-spacing before after index pure start end)
  (let* ((staves (and (%spaceable? before) (%spaceable? after)))
         (sym (if staves 'spaceable-fixed-spacing 'loose-fixed-spacing))
         (after-sp (and (ly:spanner? after) after))
         (cached (if (and pure after-sp)
                     (%spanner-cached-pure-property after-sp sym start 0)
                     '())))
    (if (number? cached)
        (%scm->double cached)
        (let* ((sys (if pure
                        (%root-system before)
                        (let ((s (ly:grob-system before))) (and (ly:grob? s) s))))
               (left (and sys staves (%system-maybe-pure-bound sys -1 pure start end)))
               (dists (if left
                          (%assoc-get 'alignment-distances
                                      (ly:grob-property left 'line-break-system-details) '())
                          '()))
               (forced (and (pair? dists) (%robust-list-ref (1- index) dists)))
               (ret (if (number? forced)
                        (let ((x (%scm->double forced))) (if (< -inf.0 x) x -inf.0))
                        -inf.0)))
          (if (and pure after-sp)
              (%spanner-cache-pure-property! after-sp sym start 0 ret))
          ret))))
(define %staff-affinities-warned #f)
(define (%staff-affinity v) (if (and (exact-integer? v) (<= -1 v 1)) v 0))
(define (%spacing-spec before after pure start end)
  (define (prop g sym)
    (if pure (%grob-pure-property g sym start end) (ly:grob-property g sym)))
  (define (unrelated g)
    (%add-stretchability (prop g 'nonstaff-unrelatedstaff-spacing) 1000000.0))
  (cond ((not (and (ly:grob? before) (ly:grob? after)))
         (%add-stretchability '() 100000000.0))
        ((%spaceable? before)
         (cond ((%spaceable? after) (prop before 'staff-staff-spacing))
               ((eqv? (%staff-affinity (prop after 'staff-affinity)) -1) (unrelated after))
               (else (prop after 'nonstaff-relatedstaff-spacing))))
        ((%spaceable? after)
         (if (eqv? (%staff-affinity (prop before 'staff-affinity)) 1)
             (unrelated before)
             (prop before 'nonstaff-relatedstaff-spacing)))
        (else
         (let* ((b (%staff-affinity (prop before 'staff-affinity)))
                (a (%staff-affinity (prop after 'staff-affinity))))
           (if (and (> a b) (not pure) (not %staff-affinities-warned))
               (begin
                 (set! %staff-affinities-warned #t)
                 (ly:warning "staff-affinities should only decrease")))
           (if (and (= b 1) (= a -1))
               (unrelated before)
               (prop before 'nonstaff-nonstaff-spacing))))))
"#;
