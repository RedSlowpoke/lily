\version "2.26.0"
% MS4 step 16: figured bass, alone and over a staff. Figures with
% alterations, augmentations, slashes and markups, brackets, `<_>`,
% implicit figures, extenders (\bassFigureExtendersOn) held across a bar
% and across a \break, centred continuations, a figure that will not
% continue (\!), and rests ignored or not; on a staff, the figures placed
% above its notes, a slur, a tie and stems among their supports.

\paper { ragged-right = ##t }

<<
  \new Staff \relative {
    \clef bass
    c4 d e f | g2( a4) b | c1~ | c2 r2 | \break
    c4 b a g | f1 | e2 d | c1 |
  }
  \figures {
    <6 4>4 <7+ 5-> <6\+ 4!> <[5 3]> |
    \bassFigureExtendersOn
    <6 4>2 <6 4>4 <6 3> | <5 3>1 | <5 3>2 <5 3> |
    <5 3>4 <6! 4> <6 4\!> <6 4> |
    \bassFigureExtendersOff
    \set figuredBassCenterContinuations = ##t
    \set useBassFigureExtenders = ##t
    <6+ 4 3>2 <6 4 3> | <_>4 <5/ 7/> <"V7" \markup \italic "b" 6> <[6 9_+ 11]>4 |
    \set implicitBassFigures = #'(5)
    <5 3>2 r2 |
  }
>>

<<
  \new Staff = bass \relative {
    \clef bass
    c4( d) e8 r8 f4~ | f4 g a b |
  }
  \context Staff = bass \figuremode {
    <4>4 <4>8 s8 <6 4>4 <6 4> |
    \set Staff.useBassFigureExtenders = ##t
    \set Staff.ignoreFiguredBassRest = ##t
    <4 6>4 <4 6> r <4 6> |
  }
>>
