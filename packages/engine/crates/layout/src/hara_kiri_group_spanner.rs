//! `hara-kiri-group-spanner.cc` (part 4's, MS3): `add_interesting_item`
//! (`hara-kiri-group-spanner.cc:238–243`, 6 lines), which
//! `Axis_group_engraver` calls with each grob of `keepAliveInterfaces`
//! when `remove-empty` is set, ported first (MS3 step 21); removing the
//! empty groups (MS3 step 27), which the staff grouper and the axis groups'
//! Scheme face call, ported first with what they call in the file:
//! `consider_suicide` (`:195–210`, 16 lines), with `unpure_request_suicide`
//! and its `_alone` (`:151–193`, 42), and `request_suicide` (`:84–102`, 19),
//! with `request_suicide_alone` and `find_in_range` (`:66–80`, `:104–149`,
//! 61). Each reads the group's `make-dead-when` and `keep-alive-with`, which
//! `Keep_alive_together_engraver` sets. And `VerticalAxisGroup`'s callbacks
//! (MS3 step 28): `ly:hara-kiri-group-spanner::y-extent`, `::calc-skylines`,
//! `::pure-height` and `::force-hara-kiri-callback` (`:32–65`, `:212–225`,
//! 40 lines), the music crate's placeholders until then. And the file's
//! last callback (MS3 step 48), `::force-hara-kiri-in-y-parent-callback`
//! (`:227–236`, 10 lines), which no grob of LilyPond's sets. All are
//! [`PRELUDE`]'s, since the callbacks and the staff grouper's Scheme face
//! call them; Rust reaches them through a [`Host`].

use lily_engine_music::Host;
use lily_engine_scheme::{BigInt, Exception, Value};

use crate::grob_array;

/// `add_interesting_item`: `n` added to the group `me`'s
/// `items-worth-living`, an unordered array
/// (`Pointer_group_interface::add_unordered_grob`).
pub fn add_interesting_item(host: &mut dyn Host, me: Value, n: Value) -> Result<(), Exception> {
    let sym = host.heap().intern("items-worth-living");
    grob_array::add_unordered_grob(host, me, sym, n)
}

/// `request_suicide`: whether the group `me` is empty between the columns
/// of the ranks `start` and `end`, as the pure heights ask, by
/// [`PRELUDE`]'s `%request-suicide`.
pub fn request_suicide(
    host: &mut dyn Host,
    me: Value,
    start: u64,
    end: u64,
) -> Result<bool, Exception> {
    let f = crate::lily_procedure(host, "%request-suicide")?;
    let heap = host.heap();
    let (start, end) = (
        heap.integer_big(BigInt::from_u64(start)),
        heap.integer_big(BigInt::from_u64(end)),
    );
    Ok(host.call(f, &[me, start, end])? != Value::FALSE)
}

/// `unpure_request_suicide`: [`request_suicide`] once the lines are
/// broken, by the grobs' liveness rather than the columns' ranks, by
/// [`PRELUDE`]'s `%unpure-request-suicide`.
pub fn unpure_request_suicide(host: &mut dyn Host, me: Value) -> Result<bool, Exception> {
    let f = crate::lily_procedure(host, "%unpure-request-suicide")?;
    Ok(host.call(f, &[me])? != Value::FALSE)
}

/// `consider_suicide`: if the group `me` is empty
/// ([`unpure_request_suicide`]), it and all its children killed, by
/// [`PRELUDE`]'s `%consider-suicide`.
pub fn consider_suicide(host: &mut dyn Host, me: Value) -> Result<(), Exception> {
    let f = crate::lily_procedure(host, "%consider-suicide")?;
    host.call(f, &[me])?;
    Ok(())
}

/// The file in Scheme: `request_suicide` with `request_suicide_alone` and
/// `find_in_range`, which the axis groups' pure staff spacing reaches
/// through the staff grouper (`staff_grouper.rs`, MS3 step 28);
/// `consider_suicide` with `unpure_request_suicide` and its `_alone`; and
/// the callbacks. Rust reaches them through a [`Host`].
///
/// - `%request-suicide me start end`: dead when a live grob of its
///   `make-dead-when` is not empty by itself; else alive when it is not
///   empty by itself, or a live grob of its `keep-alive-with` is not.
/// - `%request-suicide-alone me start end`: not empty unless
///   `remove-empty` is `#t`, nor at the first column unless `remove-first`
///   is; else empty when no rank of its `important-column-ranks` lies
///   strictly between `start` and `end`, as `vsize`s wrap. Without the
///   ranks, they are made from its `items-worth-living`' columns, sorted,
///   each once, set as `important-column-ranks`, and `%request-suicide`
///   asked again. A worthy grob of no column adds none, as its empty
///   interval does.
/// - `%find-in-range ranks low hi min max`: whether one of the sorted
///   `ranks` from `low` to before `hi` is within `min` to `max`, by
///   bisection, each rank read as it is reached (`%scm->vsize`).
/// - `%scm->vsize v`: `scm_to_size_t`, Guile's type error for no exact
///   integer, its range error for one outside 0 to `SIZE_MAX`, both from no
///   procedure.
/// - `%unpure-request-suicide me`: as `%request-suicide`, each group asked
///   `%unpure-request-suicide-alone`.
/// - `%unpure-request-suicide-alone me`: not empty unless `remove-empty`
///   is `#t`, nor, unless `remove-first` is, when its left bound's column
///   has rank 0; else empty when none of its `items-worth-living` is live.
///   Rank 0 too for no left bound, or one of no column, where LilyPond
///   would read a null column; a group that is no spanner is
///   `ly:spanner-bound`'s type error, where LilyPond 2.26.0 crashes (a
///   null `Spanner`).
/// - `%consider-suicide me`: if `me` is empty (`%unpure-request-suicide`),
///   each of its children (`%axis-group-children`, `axis_group.rs`), `me`
///   first, killed, then `me` again, as LilyPond does: once dead, a grob
///   stays as it is.
/// - The callbacks, whose type errors are named by the C++ method
///   (`ly:y-extent`, `ly:calc-skylines`, `ly:pure-height`,
///   `ly:force-hara-kiri-callback`), as LilyPond 2.26.0 names them:
///   `::y-extent` is the group's extent on Y (`%generic-group-extent`),
///   `::calc-skylines` its skylines (`%skyline-spacing`), and
///   `::force-hara-kiri-callback` 0.0, each once `%consider-suicide` has
///   been asked; `::force-hara-kiri-in-y-parent-callback` 0.0, once the
///   grob's Y parent has been asked `::force-hara-kiri-callback` (a grob
///   without one is that callback's type error, where LilyPond 2.26.0
///   reads a null); `::pure-height` is empty when the group is empty between
///   the columns (`%request-suicide`), else its pure height
///   (`%pure-group-height`), the columns 0 and `INT_MAX` where no size is
///   given (`%vsize-or`).
pub(crate) const PRELUDE: &str = r#"
(define (%scm->vsize v)
  (cond ((not (exact-integer? v))
         (scm-error 'wrong-type-arg #f "Wrong type (expecting ~A): ~S"
                    (list "exact integer" v) (list v)))
        ((not (<= 0 v 18446744073709551615))
         (scm-error 'out-of-range #f "Value out of range ~S to< ~S: ~S"
                    (list 0 18446744073709551615 v) (list v)))
        (else v)))
(define (%vsize-wrap n) (modulo n 18446744073709551616))
(define (%find-in-range ranks low hi min max)
  (if (>= low hi)
      #f
      (let* ((mid (+ low (quotient (- hi low) 2)))
             (val (%scm->vsize (vector-ref ranks mid))))
        (cond ((and (>= val min) (<= val max)) #t)
              ((< val min) (%find-in-range ranks (1+ mid) hi min max))
              (else (%find-in-range ranks low mid min max))))))
(define (%request-suicide-alone me start end)
  (cond
   ((not (eq? (ly:grob-property me 'remove-empty) #t)) #f)
   ((and (not (eq? (ly:grob-property me 'remove-first) #t)) (= start 0)) #f)
   (else
    (let ((important (ly:grob-property me 'important-column-ranks)))
      (if (vector? important)
          (cond ((or (< end 2) (< (%vsize-wrap (- end start)) 2)) #t)
                ((%find-in-range important 0 (vector-length important)
                                 (%vsize-wrap (1+ start)) (1- end))
                 #f)
                (else #t))
          (let loop ((ws (%grob-set me 'items-worth-living)) (ranks '()))
            (if (pair? ws)
                (let ((iv (ly:grob-spanned-column-rank-interval (car ws))))
                  (loop (cdr ws)
                        (do ((j (car iv) (1+ j)) (ranks ranks (cons j ranks)))
                            ((> j (cdr iv)) ranks))))
                (let unique ((rs (sort ranks <)) (out '()))
                  (cond ((pair? rs)
                         (unique (cdr rs)
                                 (if (and (pair? out) (= (car out) (car rs))) out (cons (car rs) out))))
                        (else
                         (%grob-set-property-data! me 'important-column-ranks
                                                   (list->vector (reverse! out)))
                         (%request-suicide me start end)))))))))))
(define (%request-suicide me start end)
  (let foes ((fs (%grob-set me 'make-dead-when)))
    (cond ((pair? fs)
           (if (and (%grob-live? (car fs)) (not (%request-suicide-alone (car fs) start end)))
               #t
               (foes (cdr fs))))
          ((not (%request-suicide-alone me start end)) #f)
          (else
           (let friends ((fs (%grob-set me 'keep-alive-with)))
             (cond ((null? fs) #t)
                   ((and (%grob-live? (car fs)) (not (%request-suicide-alone (car fs) start end))) #f)
                   (else (friends (cdr fs)))))))))
(define (%unpure-request-suicide-alone me)
  (cond
   ((not (eq? (ly:grob-property me 'remove-empty) #t)) #f)
   ((and (not (eq? (ly:grob-property me 'remove-first) #t))
         (let* ((l (ly:spanner-bound me -1 #f))
                (col (and (ly:grob? l) (ly:item-get-column l))))
           (or (not (ly:grob? col))
               (= (car (ly:grob-spanned-column-rank-interval col)) 0))))
    #f)
   (else
    (let worth ((ws (%grob-set me 'items-worth-living)))
      (cond ((null? ws) #t)
            ((%grob-live? (car ws)) #f)
            (else (worth (cdr ws))))))))
(define (%unpure-request-suicide me)
  (let foes ((fs (%grob-set me 'make-dead-when)))
    (cond ((pair? fs)
           (if (and (%grob-live? (car fs)) (not (%unpure-request-suicide-alone (car fs))))
               #t
               (foes (cdr fs))))
          ((not (%unpure-request-suicide-alone me)) #f)
          (else
           (let friends ((fs (%grob-set me 'keep-alive-with)))
             (cond ((null? fs) #t)
                   ((and (%grob-live? (car fs)) (not (%unpure-request-suicide-alone (car fs)))) #f)
                   (else (friends (cdr fs)))))))))
(define (%consider-suicide me)
  (when (%unpure-request-suicide me)
    (for-each ly:grob-suicide! (%axis-group-children me))
    ;; very appropriate name here :-)
    (ly:grob-suicide! me)))
(define-public (ly:hara-kiri-group-spanner::y-extent smob)
  (%check-type "ly:y-extent" 1 ly:grob? "Grob" smob)
  (%consider-suicide smob)
  (%generic-group-extent smob 1))
(define-public (ly:hara-kiri-group-spanner::calc-skylines smob)
  (%check-type "ly:calc-skylines" 1 ly:grob? "Grob" smob)
  (%consider-suicide smob)
  (%skyline-spacing smob))
(define-public (ly:hara-kiri-group-spanner::pure-height smob start end)
  (%check-type "ly:pure-height" 1 ly:grob? "Grob" smob)
  (let ((start (%vsize-or start 0))
        (end (%vsize-or end 2147483647)))
    (if (%request-suicide smob start end)
        '(+inf.0 . -inf.0)
        (%pure-group-height smob start end))))
(define-public (ly:hara-kiri-group-spanner::force-hara-kiri-callback smob)
  (%check-type "ly:force-hara-kiri-callback" 1 ly:grob? "Grob" smob)
  (%consider-suicide smob)
  0.0)
(define-public (ly:hara-kiri-group-spanner::force-hara-kiri-in-y-parent-callback smob)
  (%check-type "ly:force-hara-kiri-in-y-parent-callback" 1 ly:grob? "Grob" smob)
  (ly:hara-kiri-group-spanner::force-hara-kiri-callback (ly:grob-parent smob 1))
  0.0)
"#;
