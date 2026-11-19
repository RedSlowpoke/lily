//! `pure-from-neighbor-interface.cc` (part 3's, MS3 step 27): the callback
//! the grobs whose pure heights come from their neighbors' name,
//! `ly:pure-from-neighbor-interface::calc-pure-relevant-grobs`
//! (`pure-from-neighbor-interface.cc:30–49`, 20 lines), the music crate's
//! placeholder until then (D67). It calls part 3's
//! `Axis_group_interface::internal_calc_pure_relevant_grobs`
//! (`axis_group.rs`' `%pure-relevant-grobs`), ported first.
//!
//! The callback is the image's Scheme, as layout's others are: it reads
//! the grobs' properties through their callbacks.

/// `ly:pure-from-neighbor-interface::calc-pure-relevant-grobs`: the
/// `neighbors` of the grob's original, if it has one that is live, else
/// its own, set as the grob's `neighbors` if that is a grob array
/// (`Grob_array::set_array`, `%grob-array-set!`); then those of them that
/// are no broken piece of an item, ordered by `outside-staff-priority`
/// (`%pure-relevant-grobs`). Its type error is LilyPond's, from
/// `ly:calc-pure-relevant-grobs`, the name `MAKE_SCHEME_CALLBACK` gives
/// `LY_ASSERT_SMOB`.
pub(crate) const PRELUDE: &str = r#"
(define-public (ly:pure-from-neighbor-interface::calc-pure-relevant-grobs smob)
  (%check-type "ly:calc-pure-relevant-grobs" 1 ly:grob? "Grob" smob)
  (let* ((original (ly:grob-original smob))
         (elts (%grob-set (if (and (not (eq? original smob)) (%grob-live? original))
                              original
                              smob)
                          'neighbors))
         (arr (ly:grob-object smob 'neighbors)))
    (if (ly:grob-array? arr) (%grob-array-set! arr elts))
    (%pure-relevant-grobs smob 'neighbors)))
"#;
