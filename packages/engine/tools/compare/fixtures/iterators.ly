\version "2.26.0"
% N4's iterators that part 3's 14 scores do not reach (MS2 step 24):
% \partial, \once and \temporary overrides, \once \set and \once \revert,
% tupletSpannerDuration's splits, \time, multi-measure rests, tremolo and
% unfold repeats, \unset, \applyContext, and an override in the staff.
\relative {
  \partial 4 c'4 |
  \once \override NoteHead.color = #red
  \temporary \override Stem.color = #blue
  d8 e \once \set fontSize = #2 f g \revert Stem.color a4 b |
  \set tupletSpannerDuration = #1/4
  \tuplet 3/2 { c8 d e f g a } \tuplet 3/2 4 { b c d } c4 |
  \time 3/4 \once \revert NoteHead.color
  R2.*2 |
  \repeat tremolo 4 { c16 d } \repeat unfold 2 { e8 } \unset fontSize
  \applyContext #(lambda (c) (ly:context-set-property! c 'fontSize 1)) f4 |
  \override Staff.TimeSignature.color = #green \time 2/4 c2 |
}
