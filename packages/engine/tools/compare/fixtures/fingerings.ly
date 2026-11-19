\version "2.26.0"
% Fingering, StringNumber, StrokeFinger and FingeringColumn (MS3 step 47,
% for step 53): fingerings on single notes (Fingering_engraver) and in
% chords (New_fingering_engraver), string numbers and right-hand fingers
% on chord notes, and the fingerings set beside their chord in a column
% (fingering-column.cc, Fingering_column_engraver), each score one line.
% Fingerings above and below single notes, in chords up and down, string
% numbers and stroke fingers.
\score { \relative {
  c''4-1 d-2 e^3 f_4 | <c-1 e-3 g-5>4 <d-2 f-4>2. |
  <c\2 e\3>4 <d\4 g\1>4 <c\rightHandFinger #1 e\rightHandFinger #2 >4
  <d\rightHandFinger #3 g\rightHandFinger #4 >4 \bar "|."
} }
% Fingerings to the left and right of a chord, a column of them, string
% numbers to the right, and orientations mixed.
\score { \relative {
  \set fingeringOrientations = #'(left)
  <c'-1 e-2 g-3 c-5>4 <d-1 f-3 a-5>4
  \set fingeringOrientations = #'(right)
  <c-1 e-2 g-3>4 <e-1 g-2 c-4>4 |
  \set stringNumberOrientations = #'(right)
  <c\3 e\2 g\1>4
  \set fingeringOrientations = #'(left up down)
  <c-1 e-2 g-3 c-5>4
  \set strokeFingerOrientations = #'(left)
  <c\rightHandFinger #1 e\rightHandFinger #2 >2 \bar "|."
} }
