\version "2.26.0"
% BarNumbers over more than one staff (MS3 step 47, for step 48; OPEN
% A21): the Score's BarNumber, its side-support-elements the staves
% Staff_collecting_engraver found (stavesFound), placed above the group's
% top staff; every bar number visible, so that each is live on the line.
\layout {
  \context {
    \Score
    barNumberVisibility = #all-bar-numbers-visible
    \override BarNumber.break-visibility = #all-visible
  }
}
\score {
  \new StaffGroup <<
    \new Staff \relative { c''4 d e f | g1 | c,2 c | c1 \bar "|." }
    \new Staff \relative { \clef bass c4 b a g | f1 | e2 d | c1 }
  >>
}
% Three staves, the top one with notes above it, and the bar numbers on
% each staff instead (Bar_number_engraver in the Staff).
\score {
  <<
    \new Staff \relative { c'''4 d e f | g1 | a2 b | c1 \bar "|." }
    \new Staff \relative { c'4 d e f | g1 | a2 b | c1 }
    \new Staff \relative { \clef bass c4 b a g | f1 | e2 d | c1 }
  >>
  \layout {
    \context { \Score \remove "Bar_number_engraver" }
    \context {
      \Staff \consists "Bar_number_engraver"
      barNumberVisibility = #all-bar-numbers-visible
      \override BarNumber.break-visibility = #all-visible
    }
  }
}
