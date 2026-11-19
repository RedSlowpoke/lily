\version "2.26.0"
% Spanners broken across systems (MS3 step 60, OPEN A40): a score of five
% lines, \break between them, whose breaks are given to the engine from
% LilyPond's capture until it chooses its own (step 61). Each spanner
% crosses a break: a crescendo hairpin, an ottava bracket, a text spanner
% with its left-broken text and an arrow, a glissando sloping across
% systems and a crescendo as text in the first staff; a volta bracket,
% analysis brackets and a solid text spanner in the second. Quarter and
% half notes only: beams and ties are MS4's.
\score {
  <<
    \new Staff \relative {
      c''4 d e f | g a\< b c | \break
      c4\! \> b a g | f e d c\! |
      \ottava #1 e'4 f g a | \break
      b4 c d e \ottava #0 |
      \override TextSpanner.bound-details.left.text = "rit."
      \override TextSpanner.bound-details.left-broken.text = "(rit.)"
      \override TextSpanner.bound-details.right.arrow = ##t
      c,,4\startTextSpan d e f | \break
      g4 a b c\stopTextSpan |
      c,2\cresc c''\glissando | \break
      c,,4 d e f\! | g a b c \bar "|."
    }
    \new Voice \with { \consists "Horizontal_bracket_engraver" } \relative {
      \repeat volta 2 { c'4\startGroup d e f | }
      \alternative { { g4 a b c | c b a g\stopGroup | } { c4\startGroup b a g | } }
      f4 e d c | e f g a | b c d e\stopGroup |
      c4 d e f |
      \override TextSpanner.style = #'line
      g4\startTextSpan f e d | c d e f\stopTextSpan | g a b c \bar "|."
    }
  >>
}
