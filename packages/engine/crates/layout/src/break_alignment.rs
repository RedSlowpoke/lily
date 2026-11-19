//! `break-alignment-interface.cc` (part 3's, MS3): so far `add_element`
//! (`break-alignment-interface.cc:89–93`, 5 lines), which
//! `Break_align_engraver` calls with each `BreakAlignGroup`, ported first
//! (MS3 step 21); and `find_nonempty_break_align_group`
//! (`:95–107`, 13), which `Paper_column::break_align_width` calls for
//! `ly:staff-symbol::print` (MS3 step 27), the image's Scheme
//! ([`PRELUDE`]); and the groups' positioning (MS3 step 30), the image's
//! Scheme too: `break_align_order` (`:40–54`, 15), `ordered_elements`
//! (`:56–87`, 32) and `ly:break-alignment-interface::calc-positioning-done`
//! (`:125–288`, 164), with `ly_symbol2string` as MS2's `symbol->string`;
//! and the break-aligned interface's anchors and visibility (MS3 step 30),
//! the image's Scheme: `ly:break-aligned-interface::calc-average-anchor`
//! (`:355–413`, 59), `::calc-joint-anchor-alignment` (`:415–453`, 39),
//! `::calc-extent-aligned-anchor` (`:455–470`, 16) and
//! `::calc-break-visibility` (`:472–494`, 23), with flower's
//! `Interval::inverse_linear_combination`, `center`, `clamp` and
//! `add_point` in Scheme and `linear_combination` as
//! `%interval-linear-combination` (`self_alignment.rs`), fused as LilyPond's
//! binary computes it where these callbacks inline it.
//! And the break-alignable interface's callbacks (MS3 step 40), the
//! image's Scheme: `ly:break-alignable-interface::find-parent` with
//! `find_parent` (`:288–335`, 48) and `::self-align-callback` (`:337–353`,
//! 17), which `BarNumber`'s `X-offset` reads through output-lib.scm's
//! `self-alignment-interface::self-aligned-on-breakable`. The callback
//! `ly:break-alignment-interface::find-nonempty-break-align-group` is
//! part 3's.

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, Value};

use crate::align;

/// `add_element`: `toadd` an element of the break alignment `me`, as an
/// alignment's (`Align_interface::add_element`).
pub fn add_element(host: &mut dyn Host, me: Value, toadd: Value) -> Result<(), Exception> {
    align::add_element(host, me, toadd)
}

/// - `%find-nonempty-break-align-group`, `find_nonempty_break_align_group`:
///   the first of the break alignment `me`'s `elements` whose
///   `break-align-symbol` is `sym`, if its X extent is not empty, else
///   `#f`.
/// - `%break-align-order`, `break_align_order`: the entry of the
///   `break-align-orders` vector for `me`'s break direction, `#f` for what
///   is no vector of three entries or more.
/// - `%break-alignment-ordered-elements`, `ordered_elements`: `me`'s
///   `elements` in that order, each symbol's groups taken from the last to
///   the first and those of no symbol left out; all of them as they are
///   for an order of `#f`.
/// - `ly:break-alignment-interface::calc-positioning-done`: the groups'
///   X offsets within the column from the `space-alist` of each nonempty
///   group's last element that has one, to the next nonempty group's last
///   `break-align-symbol` (`right-edge` past the last), 1.0 of extra
///   space and LilyPond's programming error where there is no entry. The
///   groups are then aligned on the `left-edge` group, on their right
///   (with the last extra space) at the end of a line, or else on their
///   left. Where LilyPond reads through a null for what is no item, the
///   defined answer is `%check-type`'s wrong-type-arg `Item`.
/// - `%interval-add-point`, `%interval-center`, `%interval-clamp` and
///   `%interval-inverse-linear-combination`: flower's `Interval::add_point`
///   (`std::min` and `std::max`), `center`, `clamp` (as it is when empty)
///   and `inverse_linear_combination`, `(x - center) / (length * 0.5)`, a
///   length of 0 for an empty interval.
/// - `ly:break-aligned-interface::calc-average-anchor`: of the `elements`
///   whose `break-align-anchor` is a real that is no NaN, the center of
///   their anchors normalized to their own X extents (those that are
///   finite) mapped onto the group's X extent and clamped to the range of
///   their anchors; else the center of that range; else 0.
/// - `ly:break-aligned-interface::calc-joint-anchor-alignment`: the sign
///   the `elements`' nonzero `break-align-anchor-alignment`s agree on, as
///   a direction, 0 (`CENTER`) where they disagree.
/// - `ly:break-aligned-interface::calc-extent-aligned-anchor`: the
///   `break-align-anchor-alignment` combination of the grob's X extent, 0
///   where both its ends are infinite.
/// - `ly:break-aligned-interface::calc-break-visibility`: for each break
///   direction, whether one of the `elements` has a `break-visibility`
///   vector that is `#t` there (`from_scm<bool>`); Guile's out-of-range
///   error for a shorter vector, as LilyPond's `scm_c_vector_ref`.
/// - `%break-alignable-find-parent`, `Break_alignable_interface::
///   find_parent`, and `ly:break-alignable-interface::find-parent`: of
///   the break alignment that is the grob's X parent, the first group in
///   its order of the first of the grob's `break-align-symbols` that has
///   one which is visible at its break and not empty on X; else the first
///   group met of any of them; `#f` for none, or a parent that is no break
///   alignment.
/// - `ly:break-alignable-interface::self-align-callback`: the X offset
///   that puts the grob at that group's `break-align-anchor` (0 where it
///   is no number), from their common refpoint; the exact 0 without one.
pub(crate) const PRELUDE: &str = r#"
(define (%find-nonempty-break-align-group me sym)
  (let loop ((groups (%grob-set me 'elements)))
    (cond ((null? groups) #f)
          ((eq? (ly:grob-property (car groups) 'break-align-symbol) sym)
           (let ((e (%grob-extent (car groups) (car groups) 0)))
             (and (not (> (car e) (cdr e))) (car groups))))
          (else (loop (cdr groups))))))
(define (%break-align-order me)
  (let ((orders (ly:grob-property me 'break-align-orders)))
    (if (and (vector? orders) (>= (vector-length orders) 3))
        (vector-ref orders (1+ (ly:item-break-dir me)))
        #f)))
(define (%break-alignment-ordered-elements me)
  (let ((elts (%grob-set me 'elements))
        (order (%break-align-order me)))
    (if (not order)
        elts
        (let loop ((order order) (writable (reverse elts)) (taken '()))
          (if (pair? order)
              (let split ((gs writable) (kept '()) (taken taken))
                (cond ((null? gs) (loop (cdr order) (reverse kept) taken))
                      ((eq? (car order) (ly:grob-property (car gs) 'break-align-symbol))
                       (split (cdr gs) kept (cons (car gs) taken)))
                      (else (split (cdr gs) (cons (car gs) kept) taken))))
              (reverse taken))))))
(define-public (ly:break-alignment-interface::calc-positioning-done smob)
  (%check-type "ly:break-alignment-interface::calc-positioning-done" 1 ly:item? "Item" smob)
  (%grob-set-property-data! smob 'positioning-done #t)
  (let* ((elems (list->vector (%break-alignment-ordered-elements smob)))
         (n (vector-length elems))
         (extents (list->vector (map (lambda (g) (%grob-extent g g 0)) (vector->list elems))))
         (nonempty (lambda (i)
                     (let skip ((i i))
                       (if (and (< i n) (%interval-empty? (vector-ref extents i))) (skip (1+ i)) i))))
         (offsets (make-vector n 0.0)))
    (let walk ((idx (nonempty 0)) (extra-right-space 0.0) (edge-idx #f))
      (if (< idx n)
          (let* ((next-idx (nonempty (1+ idx)))
                 (l (vector-ref elems idx))
                 (r (and (< next-idx n) (vector-ref elems next-idx)))
                 (found (let scan ((es (reverse (%grob-set l 'elements))) (edge-idx edge-idx))
                          (if (null? es)
                              (cons '() edge-idx)
                              (let* ((elt (car es))
                                     (edge-idx (if (and (not edge-idx)
                                                        (eq? (ly:grob-property elt 'break-align-symbol)
                                                             'left-edge))
                                                   idx
                                                   edge-idx))
                                     (alist (ly:grob-property elt 'space-alist)))
                                (if (pair? alist) (cons alist edge-idx) (scan (cdr es) edge-idx))))))
                 (alist (car found))
                 (rsym (if r
                           (let scan ((es (reverse (%grob-set r 'elements))) (rsym '()))
                             (if (or (symbol? rsym) (null? es))
                                 rsym
                                 (scan (cdr es) (ly:grob-property (car es) 'break-align-symbol))))
                           'right-edge))
                 (edge-idx (if (eq? rsym 'left-edge) next-idx (cdr found)))
                 (entry (if (symbol? rsym) (assq rsym alist) '()))
                 (entry-found (pair? entry)))
            (if (not entry-found)
                (let ((cause (ly:grob-property l 'cause)))
                  (ly:programming-error "No spacing entry from ~a to `~a'"
                                        (if (ly:grob? cause) (%grob-name cause) "")
                                        (if (symbol? rsym) (symbol->string rsym) ""))))
            (let* ((entry (and entry-found (cdr entry)))
                   (distance (if entry-found (%scm->double (cdr entry)) 1.0))
                   (type (if entry-found (car entry) 'extra-space)))
              (if r
                  (cond ((eq? type 'extra-space)
                         (vector-set! offsets next-idx
                                      (- (+ (cdr (vector-ref extents idx)) distance)
                                         (car (vector-ref extents next-idx)))))
                        ((eq? type 'minimum-space)
                         (vector-set! offsets next-idx
                                      (%std-max (cdr (vector-ref extents idx)) distance))))
                  (if (< (1+ idx) n)
                      (vector-set! offsets (1+ idx) (+ (cdr (vector-ref extents idx)) distance))))
              (walk next-idx (if r extra-right-space distance) edge-idx)))
          (let total ((i 0) (here 0.0) (extent '(+inf.0 . -inf.0)) (alignment-off 0.0))
            (if (< i n)
                (let* ((here (+ here (vector-ref offsets i)))
                       (e (vector-ref extents i)))
                  (total (1+ i) here
                         (%interval-unite extent (cons (+ (car e) here) (+ (cdr e) here)))
                         (if (eqv? i edge-idx) (- here) alignment-off)))
                (if (not (%interval-empty? extent))
                    (let ((alignment-off (cond ((eqv? (ly:item-break-dir smob) -1)
                                                (- (- (cdr extent)) extra-right-space))
                                               ((not edge-idx) (- (car extent)))
                                               (else alignment-off))))
                      (let translate ((i 0) (here alignment-off))
                        (if (< i n)
                            (let ((here (+ here (vector-ref offsets i))))
                              (ly:grob-translate-axis! (vector-ref elems i) here 0)
                              (translate (1+ i) here))))))))))
    #t))
(define (%interval-add-point i p)
  (cons (if (< p (car i)) p (car i)) (if (< (cdr i) p) p (cdr i))))
(define (%interval-center i) (* (+ (car i) (cdr i)) 0.5))
(define (%interval-clamp i t)
  (cond ((%interval-empty? i) t)
        ((< t (car i)) (car i))
        ((> t (cdr i)) (cdr i))
        (else t)))
(define (%interval-inverse-linear-combination i x)
  (/ (- x (%interval-center i))
     (* (if (%interval-empty? i) 0.0 (- (cdr i) (car i))) 0.5)))
(define-public (ly:break-aligned-interface::calc-average-anchor grob)
  (%check-type "ly:break-aligned-interface::calc-average-anchor" 1 ly:grob? "Grob" grob)
  (let loop ((elts (%grob-set grob 'elements))
             (absolute '(+inf.0 . -inf.0))
             (normalized '(+inf.0 . -inf.0)))
    (if (pair? elts)
        (let* ((g (car elts))
               (anchor (%double-or (ly:grob-property g 'break-align-anchor) +nan.0)))
          (if (nan? anchor)
              (loop (cdr elts) absolute normalized)
              (let ((n (%interval-inverse-linear-combination (%grob-extent g g 0) anchor)))
                (loop (cdr elts)
                      (%interval-add-point absolute anchor)
                      (if (finite? n) (%interval-add-point normalized n) normalized)))))
        (cond ((not (%interval-empty? normalized))
               (%interval-clamp absolute
                                (%interval-linear-combination (%grob-extent grob grob 0)
                                                              (%interval-center normalized))))
              ((not (%interval-empty? absolute)) (%interval-center absolute))
              (else 0.0)))))
(define-public (ly:break-aligned-interface::calc-joint-anchor-alignment grob)
  (%check-type "ly:break-aligned-interface::calc-joint-anchor-alignment" 1 ly:grob? "Grob" grob)
  (let loop ((elts (%grob-set grob 'elements)) (direction 0))
    (if (pair? elts)
        (let ((alignment (%double-or (ly:grob-property (car elts) 'break-align-anchor-alignment)
                                     0.0)))
          (cond ((< alignment 0) (if (> direction 0) 0 (loop (cdr elts) -1)))
                ((> alignment 0) (if (< direction 0) 0 (loop (cdr elts) 1)))
                (else (loop (cdr elts) direction))))
        direction)))
(define-public (ly:break-aligned-interface::calc-extent-aligned-anchor smob)
  (%check-type "ly:break-aligned-interface::calc-extent-aligned-anchor" 1 ly:grob? "Grob" smob)
  (let ((alignment (%double-or (ly:grob-property smob 'break-align-anchor-alignment) 0.0))
        (iv (%grob-extent smob smob 0)))
    (if (and (inf? (car iv)) (inf? (cdr iv)))
        0.0
        (%interval-linear-combination iv alignment))))
(define-public (ly:break-aligned-interface::calc-break-visibility smob)
  (%check-type "ly:break-aligned-interface::calc-break-visibility" 1 ly:grob? "Grob" smob)
  (let ((elts (%grob-set smob 'elements)))
    (list->vector
     (map (lambda (dir)
            (let loop ((es elts) (visible #f))
              (if (pair? es)
                  (let ((vis (ly:grob-property (car es) 'break-visibility)))
                    (loop (cdr es)
                          (or (and (vector? vis) (eq? (vector-ref vis dir) #t)) visible)))
                  visible)))
          '(0 1 2)))))
(define (%break-alignable-find-parent me)
  (let ((alignment (ly:grob-parent me 0)))
    (and (ly:item? alignment)
         (%grob-has-interface? alignment 'break-alignment-interface)
         (let ((elements (%break-alignment-ordered-elements alignment)))
           (and (pair? elements)
                (let loop ((syms (ly:grob-property me 'break-align-symbols)) (found #f))
                  (if (pair? syms)
                      (let scan ((gs elements) (found found))
                        (cond ((null? gs) (loop (cdr syms) found))
                              ((and (eq? (car syms) (ly:grob-property (car gs) 'break-align-symbol))
                                    (ly:item? (car gs)))
                               (let ((it (car gs)))
                                 (if (and (%item-break-visible? it)
                                          (not (%interval-empty? (%grob-extent it it 0))))
                                     it
                                     (scan (cdr gs) (or found it)))))
                              (else (scan (cdr gs) found))))
                      found)))))))
(define-public (ly:break-alignable-interface::find-parent grob)
  (%check-type "ly:break-alignable-interface::find-parent" 1 ly:grob? "Grob" grob)
  (%break-alignable-find-parent grob))
(define-public (ly:break-alignable-interface::self-align-callback grob)
  (%check-type "ly:break-alignable-interface::self-align-callback" 1 ly:grob? "Grob" grob)
  (let ((parent (%break-alignable-find-parent grob)))
    (if (not parent)
        0
        (let* ((common (ly:grob-common-refpoint grob parent 0))
               (anchor (%double-or (ly:grob-property parent 'break-align-anchor) 0.0))
               (there (%grob-relative-coordinate parent common 0)))
          (+ (- there (%grob-relative-coordinate grob common 0)) anchor)))))
"#;
