//! `staff-grouper-interface.cc` (part 3's, MS3 step 27): what the vertical
//! alignment and the axis groups' staff spacing (step 28's) ask of a
//! `StaffGrouper` or a `VerticalAlignment` about the staves among its
//! `elements`: [`get_extremal_staff`] (`staff-grouper-interface.cc:28–56`,
//! 29 lines) and [`maybe_pure_within_group`] (`:58–83`, 26). They remove
//! empty groups and tell staves from loose lines with part 4's
//! `Hara_kiri_group_spanner::consider_suicide` and `request_suicide` and
//! part 5's `Page_layout_problem::is_spaceable`, ported first
//! (`hara_kiri_group_spanner.rs`, `page_layout_problem.rs`). Neither has a
//! Scheme name; the interface has no callback. `maybe_pure_within_group`
//! is [`PRELUDE`]'s since the axis groups' staff spacing calls it (MS3
//! step 28); Rust reaches it through a [`Host`].

use lily_engine_music::Host;
use lily_engine_scheme::{BigInt, Exception, Value};

use crate::geometry::{Direction, Interval, X};
use crate::grob::{self, is_live_grob};
use crate::grob_array::extract_grob_array;
use crate::hara_kiri_group_spanner::consider_suicide;

/// `get_extremal_staff`: the furthest of `me`'s `elements` in the direction
/// `dir`, from the first for `UP` (1) or from the last for `DOWN`, that is
/// live and whose extent on X relative to `refpoint` meets `iv`. Each
/// hara-kiri group reached is asked to remove itself first
/// (`consider_suicide`), and each element's extent is read before its
/// liveness, as LilyPond does.
pub fn get_extremal_staff(
    host: &mut dyn Host,
    me: Value,
    refpoint: Value,
    dir: Direction,
    iv: Interval,
) -> Result<Option<Value>, Exception> {
    let elts = extract_grob_array(host, me, "elements")?;
    let order: alloc::vec::Vec<Value> = if dir == 1 {
        elts
    } else {
        elts.into_iter().rev().collect()
    };
    for e in order {
        if grob::has_interface(host.heap(), e, "hara-kiri-group-spanner-interface") {
            consider_suicide(host, e)?;
        }
        let mut intersection = grob::extent(host, e, refpoint, X)?;
        intersection.intersect(iv);
        if is_live_grob(host.heap(), e) && !intersection.is_empty() {
            return Ok(Some(e));
        }
    }
    Ok(None)
}

/// `maybe_pure_within_group`: whether `child`, one of `me`'s `elements`, is
/// within the group, by [`PRELUDE`]'s `%staff-grouper-within-group?`.
pub fn maybe_pure_within_group(
    host: &mut dyn Host,
    me: Value,
    child: Value,
    pure: bool,
    start: u64,
    end: u64,
) -> Result<bool, Exception> {
    let f = crate::lily_procedure(host, "%staff-grouper-within-group?")?;
    let heap = host.heap();
    let (start, end) = (
        heap.integer_big(BigInt::from_u64(start)),
        heap.integer_big(BigInt::from_u64(end)),
    );
    let v = host.call(f, &[me, child, Value::bool(pure), start, end])?;
    Ok(v != Value::FALSE)
}

/// `maybe_pure_within_group` in Scheme, which the axis groups' staff
/// spacing calls (`axis_group.rs`, MS3 step 28), with `is_spaceable`:
///
/// - `%spaceable? g`: `g` has no number as its `staff-affinity`.
/// - `%staff-grouper-within-group? me child pure start end`: whether the
///   next of `me`'s `elements` after `child` that is a staff (`%spaceable?`)
///   and alive has `me` as its `staff-grouper`. Alive is, with `pure`, not
///   empty between the columns of the ranks `start` and `end`
///   (`%request-suicide`, `hara_kiri_group_spanner.rs`), else live. False
///   for a child that is no element, or with no such staff after it.
pub(crate) const PRELUDE: &str = r#"
(define (%spaceable? g)
  (not (number? (ly:grob-property g 'staff-affinity))))
(define (%staff-grouper-within-group? me child pure start end)
  (let ((tail (memq child (%grob-set me 'elements))))
    (and tail
         (let loop ((es (cdr tail)))
           (cond ((null? es) #f)
                 ((and (%spaceable? (car es))
                       (if pure
                           (not (%request-suicide (car es) start end))
                           (%grob-live? (car es))))
                  (eq? (ly:grob-object (car es) 'staff-grouper) me))
                 (else (loop (cdr es))))))))
"#;
