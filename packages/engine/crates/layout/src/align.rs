//! `align-interface.cc` (part 3's, MS3): so far `get_pure_child_y_translation`
//! (`align-interface.cc:322–342`, 21 lines), which `grob.cc`'s
//! `pure_relative_y_coordinate` calls for the child of a vertical
//! alignment, ported with the grob's parents and offsets (MS3 step 13).
//! It is [`PRELUDE`]'s, so that the pure coordinate, which calls the
//! offsets' callbacks there, reaches it (`unpure.rs`, MS3 step 14); Rust
//! reaches it through a [`Host`].
//!
//! The alignment's `elements` are read as `extract_grob_set` reads them
//! (`grob_array.rs`, MS3 step 14).
//!
//! And `add_element` (`align-interface.cc:350–359`, 10 lines), which
//! `Vertical_align_engraver` and, through `break_alignment.rs`,
//! `Break_align_engraver` call, ported first (MS3 step 21), with the
//! `axis` it reads (`:344–348`, 5 lines).
//!
//! And the rest (MS3 step 28, [`PRELUDE`]): the callbacks
//! `ly:align-interface::align-to-minimum-distances` and
//! `::align-to-ideal-distances` (`:34–60`, `:298–320`), the music crate's
//! placeholders until then, with `get_skylines` and
//! `internal_get_minimum_translations` (`:62–296`), the elements stacked
//! by their skylines and spacing specs, which `get_pure_minimum_translations`
//! (`:136–143`) gives the pure coordinate. The ideal distances are the
//! page-layout problem's of the alignment's system
//! (`page_layout_problem.rs`).

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, Heap, Tag, Tracer, Value};

use crate::R;
use crate::axis_group;
use crate::geometry::{Axis, X, Y};
use crate::grob::{self, grobs};

/// `axis`: the alignment `me`'s axis, the first of its `axes`, X where
/// that is no axis (`from_scm<Axis> (…, X_AXIS)`). No axes is Guile's type
/// error from `car`, where LilyPond 2.26.0 crashes (`scm_car` unchecked).
pub fn axis(host: &mut dyn Host, me: Value) -> Result<Axis, Exception> {
    let axes = grob::property(host, me, "axes")?;
    let heap = host.heap();
    match heap.car(axes) {
        Some(a) => Ok(if a.as_fixnum() == Some(1) { Y } else { X }),
        None => Err(crate::wrong_type(heap, Some("car"), 0, axes, "pair")),
    }
}

/// `add_element`: `element` an element of the alignment `me`, its offset
/// on `me`'s axis `ly:grob::x-parent-positioning` or
/// `ly:grob::y-parent-positioning` (`axis_parent_positioning`), then an
/// element of `me` as an axis group's (`axis_group.rs`).
pub fn add_element(host: &mut dyn Host, me: Value, element: Value) -> Result<(), Exception> {
    let a = axis(host, me)?;
    let (sym, proc) = if a == X {
        ("X-offset", "ly:grob::x-parent-positioning")
    } else {
        ("Y-offset", "ly:grob::y-parent-positioning")
    };
    let proc = crate::lily_procedure(host, proc)?;
    grob::set_property(host.heap(), element, sym, proc);
    axis_group::add_element(host, me, element)
}

/// `get_pure_child_y_translation`: the pure translation on Y of `ch` among
/// the alignment `me`'s elements, between the columns `start` and `end`;
/// 0 if the elements have none, and LilyPond's programming error, and 0,
/// for no element of `me`.
pub fn get_pure_child_y_translation(
    host: &mut dyn Host,
    me: Value,
    ch: Value,
    start: usize,
    end: usize,
) -> Result<f64, Exception> {
    crate::unpure::read_translation();
    let heap = host.heap();
    let fresh = is_fresh(heap, me, (start as i64, end as i64), false);
    let (start, end) = (heap.integer(start as i64), heap.integer(end as i64));
    if !fresh && let Some(t) = kept_child_y_translation(host, me, ch, start, end)? {
        return Ok(t);
    }
    let f = crate::lily_procedure(host, "%align-pure-child-y-translation")?;
    // A cache whoever asks first makes: what it reads to make it is not what
    // the grob that asked depends on, the translation is (`cell.rs`' PURE_Y).
    let outer = crate::cell::outside(host.heap(), me);
    let v = host.call(f, &[me, ch, start, end]);
    crate::cell::inside(host.heap(), me, outer);
    Ok(host.heap().to_f64(v?).unwrap_or(0.0))
}

/// `%align-pure-child-y-translation` where the alignment `me` has kept
/// its translations between `start` and `end` (MS4 step 21): `ch`'s,
/// found among its elements as the Scheme finds it, with no VM call and
/// nothing consed. `None` leaves it to the Scheme, to compute them or to
/// give its error or programming error: none kept, `ch` no element, fewer
/// translations than elements before it, or one no fixnum or flonum.
fn kept_child_y_translation(
    host: &mut dyn Host,
    me: Value,
    ch: Value,
    start: Value,
    end: Value,
) -> Result<Option<f64>, Exception> {
    let elements = crate::grob_array::extract_grob_array(host, me, "elements")?;
    let alist = grob::property(host, me, "minimum-translations-alist")?;
    let heap = host.heap();
    let (Some(start), Some(end)) = (start.as_fixnum(), end.as_fixnum()) else {
        return Ok(None);
    };
    let Some(cached) = grob_of(heap, me).and_then(|(arena, id)| {
        grobs(&mut arena.borrow_mut())
            .get_mut(id)?
            .translations
            .assoc(heap, (start, end), alist)
    }) else {
        return Ok(None);
    };
    let mut translates = match heap.cdr(cached) {
        Some(t) if t != Value::NULL => t,
        _ => return Ok(None),
    };
    let mut all = Vec::with_capacity(elements.len());
    while translates != Value::NULL {
        match heap.car(translates) {
            Some(t) if matches!(t.tag(), Tag::Fixnum | Tag::Flonum) => all.push(t),
            _ => return Ok(None),
        }
        translates = match heap.cdr(translates) {
            Some(t) => t,
            None => return Ok(None),
        };
    }
    Ok(match elements.iter().position(|&g| g == ch) {
        Some(i) if i < all.len() => heap.to_f64(all[i]),
        _ => None,
    })
}

/// An alignment's `minimum-translations-alist` by its keys (MS4 step 1),
/// which the breakers would otherwise search with `assoc` for each
/// candidate line: the first entry of each key that is a pair of
/// fixnums, which is what `(assoc key alist)` finds for such a key, kept
/// for the alist `head`. An entry of another key cannot be `equal?` to
/// one, and is left out.
#[derive(Clone)]
pub(crate) struct Translations {
    head: Value,
    by_key: BTreeMap<(i64, i64), Value>,
    /// The columns whose translations the next ask makes again, not found
    /// (B28, D272): a held line of marks goes on where LilyPond's nested
    /// read had asked them before the lines below were read.
    fresh: Option<(i64, i64)>,
}

impl Default for Translations {
    /// The keys of `'()`.
    fn default() -> Translations {
        Translations {
            head: Value::NULL,
            by_key: BTreeMap::new(),
            fresh: None,
        }
    }
}

impl Translations {
    /// Whether it holds the same alist and entries (N8's splice).
    pub(crate) fn same(&self, other: &Translations) -> bool {
        self.head == other.head && self.by_key == other.by_key
    }

    pub(crate) fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.head);
        for &entry in self.by_key.values() {
            tracer.trace(entry);
        }
    }

    /// What `(assoc key alist)` gives for `key`, a pair of fixnums: the
    /// entry, or `#f`. The keys are those of `head` when `alist` is it, or
    /// it with one entry consed on, as the pure translations add one; else
    /// read again. `None` where `assoc` would find what is no proper list
    /// of pairs, whose error it gives.
    fn assoc(&mut self, heap: &Heap, key: (i64, i64), alist: Value) -> Option<Value> {
        if alist != self.head {
            if heap.cdr(alist) == Some(self.head) {
                let entry = heap.car(alist).filter(|&e| heap.car(e).is_some())?;
                if let Some(k) = fixnum_pair(heap, heap.car(entry)?) {
                    self.by_key.insert(k, entry);
                }
            } else {
                let mut by_key = BTreeMap::new();
                let mut p = alist;
                while p != Value::NULL {
                    let entry = heap.car(p).filter(|&e| heap.car(e).is_some())?;
                    if let Some(k) = fixnum_pair(heap, heap.car(entry)?) {
                        by_key.entry(k).or_insert(entry);
                    }
                    p = heap.cdr(p)?;
                }
                self.by_key = by_key;
            }
            self.head = alist;
        }
        Some(self.by_key.get(&key).copied().unwrap_or(Value::FALSE))
    }
}

/// Whether the alignment `me` makes its translations between the columns
/// `key` again at the next ask (B28, D272); `take` ends it.
fn is_fresh(heap: &Heap, me: Value, key: (i64, i64), take: bool) -> bool {
    let Some((arena, id)) = grob_of(heap, me) else {
        return false;
    };
    let mut a = arena.borrow_mut();
    let Some(g) = grobs(&mut a).get_mut(id) else {
        return false;
    };
    let fresh = g.translations.fresh == Some(key);
    if fresh && take {
        g.translations.fresh = None;
    }
    fresh
}

/// The alignment `me` makes its translations between `key`'s columns again
/// at the next ask, or (`None`) as it finds them.
pub(crate) fn make_fresh(heap: &Heap, me: Value, key: Option<(i64, i64)>) {
    if let Some((arena, id)) = grob_of(heap, me)
        && let Some(g) = grobs(&mut arena.borrow_mut()).get_mut(id)
    {
        g.translations.fresh = key;
    }
}

/// Whether the alignment `me` has kept its pure translations between
/// `key`'s columns, as `%align-minimum-translations` finds them.
pub(crate) fn translations_kept(
    host: &mut dyn Host,
    me: Value,
    key: (i64, i64),
) -> Result<bool, Exception> {
    let alist = grob::property(host, me, "minimum-translations-alist")?;
    let heap = host.heap();
    let found = grob_of(heap, me).and_then(|(arena, id)| {
        grobs(&mut arena.borrow_mut())
            .get_mut(id)?
            .translations
            .assoc(heap, key, alist)
    });
    Ok(found.is_some_and(|e| heap.cdr(e).is_some_and(|t| t != Value::NULL)))
}

/// The two fixnums of the pair `v`.
fn fixnum_pair(heap: &Heap, v: Value) -> Option<(i64, i64)> {
    Some((heap.car(v)?.as_fixnum()?, heap.cdr(v)?.as_fixnum()?))
}

/// `(%align-translations-assoc me key alist)`: `(assoc key alist)` for
/// the alignment `me`'s `minimum-translations-alist` `alist`, by its keys
/// ([`Translations`]) when `key` is a pair of fixnums; by `assoc` else.
pub(crate) fn translations_assoc(cx: &mut Cx, args: &[Value]) -> R {
    let heap = cx.heap();
    if let Some(key) = fixnum_pair(heap, args[1])
        && is_fresh(heap, args[0], key, true)
    {
        return Ok(Value::FALSE);
    }
    if let Some(key) = fixnum_pair(heap, args[1])
        && let Some((arena, id)) = grob_of(heap, args[0])
        && let Some(g) = grobs(&mut arena.borrow_mut()).get_mut(id)
        && let Some(found) = g.translations.assoc(heap, key, args[2])
    {
        return Ok(found);
    }
    match cx.global("lily", "assoc") {
        Some(f) => Ok(cx.tail_call(f, &[args[1], args[2]])),
        None => Ok(Value::FALSE),
    }
}

/// The alignment in Scheme:
///
/// - `%align-axis me`, [`axis`].
/// - `%align-skylines g a other-common pure start end`, `get_skylines`:
///   unpure, `g`'s skylines on `a` moved by its coordinate on the other
///   axis from `other-common`, none if it has none; pure, none for a group
///   that `%request-suicide`s, else the box of its pure height from X 0 on,
///   an axis group's the rest of the line's, and its height at the
///   beginning of a line before X −1.
/// - `%align-minimum-translations me elems a include-fixed pure start end`,
///   `internal_get_minimum_translations`: each element's translation on
///   `a` from the first, by `stacking-dir` (`DOWN` for none), each placed
///   past the skylines of those before it by the `padding` of its spacing
///   spec from the last non-empty one (the alignment's `padding` without
///   one) and at least its `minimum-distance`, and its `basic-distance`
///   between the columns 0 and `INT_MAX`; an empty one at the last one's.
///   With `include-fixed`, for the system's alignment only, a staff is
///   placed past the last staff as well, and at least at its fixed
///   spacing. Pure, kept in `minimum-translations-alist` by `(start .
///   end)`, found by its keys ([`translations_assoc`]). Unpure on Y, a spanner of no system is LilyPond's programming
///   error. A `stacking-dir` of 0 (`CENTER`), which LilyPond's skylines
///   cannot take, stacks no element but bounds the skylines as `DOWN`.
/// - `%align-pure-minimum-translations me all-grobs a start end`,
///   `get_pure_minimum_translations`.
/// - `get_pure_child_y_translation`, the elements read as
///   `extract_grob_set` reads them.
/// - `ly:align-interface::align-to-minimum-distances`: each element moved
///   by its translation, fixed spacing included.
/// - `ly:align-interface::align-to-ideal-distances`: the page-layout
///   problem of the alignment's system solved, ragged
///   (`%page-layout-solution`); LilyPond's programming error without a
///   system.
///
/// Both set `positioning-done` as C++ `set_property` does
/// (`%grob-set-property-data!`), as the pure translations set
/// `minimum-translations-alist`.
pub(crate) const PRELUDE: &str = r#"
(define (%align-axis me)
  (let ((axes (ly:grob-property me 'axes)))
    (if (pair? axes)
        (if (eqv? (car axes) 1) 1 0)
        (scm-error 'wrong-type-arg "car" "Wrong type argument (expecting ~A): ~S"
                   (list "pair" axes) (list axes)))))
(define (%std-max a b) (if (< a b) b a))
(define (%align-skylines g a other-common pure start end)
  (cond ((not pure)
         (let ((skyp (ly:grob-property g (if (eqv? a 1) 'vertical-skylines 'horizontal-skylines))))
           (if (%skyline-pair? skyp)
               (%skyline-pair-shifted skyp (%grob-relative-coordinate g other-common (- 1 a)))
               (%boxes-skylines '()))))
        ((%request-suicide g start end) (%boxes-skylines '()))
        (else
         (let* ((extent (%grob-pure-y-extent g g start end))
                (group (%grob-has-interface? g 'axis-group-interface))
                (extent (if group (%part-of-line-pure-height g #f start end) extent))
                (begin-extent (and group (%part-of-line-pure-height g #t start (1+ start))))
                (boxes (if (and begin-extent (not (%interval-empty? begin-extent)))
                           (list (cons '(-inf.0 . -1.0) begin-extent))
                           '())))
           (%boxes-skylines (if (%interval-empty? extent)
                                boxes
                                (append boxes (list (cons '(0.0 . +inf.0) extent)))))))))
(define (%align-minimum-translations me elems a include-fixed pure start end)
  (if (and (not pure) (eqv? a 1) (ly:spanner? me) (not (ly:grob? (ly:grob-system me))))
      (%grob-programming-error me "vertical alignment called before line-breaking"))
  (let* ((key (cons start end))
         (cached (and pure (%align-translations-assoc
                            me key (ly:grob-property me 'minimum-translations-alist)))))
    (if (and cached (not (null? (cdr cached))))
        (map %scm->double (cdr cached))
        (let* ((include-fixed (and include-fixed (%system? (ly:grob-parent me 1))))
               (d (let ((d (ly:grob-property me 'stacking-dir)))
                    (if (and (exact-integer? d) (<= -1 d 1)) d -1)))
               (dir (exact->inexact d))
               (sky (if (= d 1) cdr car))
               (sky-other (if (= d 1) car cdr))
               (other-common (let ((c (ly:grob-common-refpoint-of-array
                                       me (ly:grob-list->grob-array elems) (- 1 a))))
                               (if (ly:grob? c) c '())))
               (default-padding (%real-or (ly:grob-property me 'padding) 0.0)))
          (let loop ((es elems) (where 0.0) (translates '())
                     (down (%skyline-empty d)) (last-nonempty #f)
                     (last-spaceable-pos 0.0) (last-spaceable #f)
                     (last-spaceable-sky (%skyline-empty d)) (spaceable-count 0))
            (if (null? es)
                (let ((translates (reverse translates)))
                  (if pure
                      (%grob-set-property-data!
                       me 'minimum-translations-alist
                       (cons (cons key translates) (ly:grob-property me 'minimum-translations-alist))))
                  translates)
                (let* ((e (car es))
                       (skyline (begin (%queue-check 18) (%align-skylines e a other-common pure start end))))
                  (if (%skyline-pair-empty? skyline)
                      (loop (cdr es) where (cons where translates) down last-nonempty
                            last-spaceable-pos last-spaceable last-spaceable-sky spaceable-count)
                      (let* ((facing (sky-other skyline))
                             (dy (if (not last-nonempty)
                                     (+ (%skyline-max-height facing) default-padding)
                                     (let* ((spec (%spacing-spec last-nonempty e pure start end))
                                            (padding (or (%read-spacing-spec spec 'padding) default-padding))
                                            (dy (+ (%skyline-distance down facing 0.0) padding))
                                            (min-dist (%read-spacing-spec spec 'minimum-distance))
                                            (dy (if min-dist (%std-max dy min-dist) dy))
                                            (basic (and (eqv? end 2147483647) (eqv? start 0)
                                                        (%read-spacing-spec spec 'basic-distance)))
                                            (dy (if basic (%std-max dy basic) dy)))
                                       (if (and include-fixed (%spaceable? e) last-spaceable)
                                           (let* ((spec (%spacing-spec last-spaceable e pure start end))
                                                  (sp-padding (or (%read-spacing-spec spec 'padding) 0.0))
                                                  (shift (* dir (- last-spaceable-pos where)))
                                                  (dy (%std-max dy (+ (+ (%skyline-distance
                                                                          last-spaceable-sky facing 0.0)
                                                                         shift)
                                                                      sp-padding)))
                                                  (sp-min (%read-spacing-spec spec 'minimum-distance))
                                                  (dy (if sp-min (%std-max dy (+ sp-min shift)) dy)))
                                             (%std-max dy (%fixed-spacing last-spaceable e spaceable-count
                                                                          pure start end)))
                                           dy))))
                             (translates (if last-nonempty
                                             translates
                                             (map (lambda (t) (* dir dy)) translates)))
                             (dy (%std-max 0.0 dy))
                             (down (%skyline-merged (%skyline-raised down (* (- dir) dy)) (sky skyline)))
                             (where (+ where (* dir dy)))
                             (spaceable (%spaceable? e)))
                        (loop (cdr es) where (cons where translates) down e
                              (if spaceable where last-spaceable-pos)
                              (if spaceable e last-spaceable)
                              (if spaceable down last-spaceable-sky)
                              (if spaceable (1+ spaceable-count) spaceable-count)))))))))))
(define (%align-pure-minimum-translations me all-grobs a start end)
  (%align-minimum-translations me all-grobs a #t #t start end))
(define (%align-pure-child-y-translation me ch start end)
  (let* ((elements (ly:grob-object me 'elements))
         (all-grobs (if (ly:grob-array? elements) (ly:grob-array->list elements) '()))
         (translates (%align-pure-minimum-translations me all-grobs 1 start end)))
    (if (null? translates)
        0.0
        (let loop ((grobs all-grobs) (translates translates))
          (cond ((null? grobs)
                 (ly:programming-error
                  "tried to get a translation for something that is no child of mine")
                 0.0)
                ((eq? (car grobs) ch) (car translates))
                (else (loop (cdr grobs) (cdr translates))))))))
(define-public (ly:align-interface::align-to-minimum-distances smob)
  (%check-type "ly:align-to-minimum-distances" 1 ly:grob? "Grob" smob)
  (%grob-set-property-data! smob 'positioning-done #t)
  (let* ((a (%align-axis smob))
         (all-grobs (%grob-set smob 'elements))
         (translates (%align-minimum-translations smob all-grobs a #t #f 0 0)))
    (if (pair? translates)
        (for-each (lambda (g t) (ly:grob-translate-axis! g t a)) all-grobs translates)))
  #t)
(define-public (ly:align-interface::align-to-ideal-distances smob)
  (%check-type "ly:align-to-ideal-distances" 1 ly:grob? "Grob" smob)
  (%grob-set-property-data! smob 'positioning-done #t)
  (let ((sys (ly:grob-system smob)))
    (if (ly:grob? sys)
        (%page-layout-solution #f #f (list sys) #t)
        (ly:programming-error "vertical alignment called before line breaking")))
  #t)
"#;
