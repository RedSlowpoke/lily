\version "2.26.0"
motif = \relative { c''8 d e4 }
\new Staff \relative {
  \clef treble
  g'4 a8 b <c e g>2 |
  \motif \transpose c f \motif r4 |
  \new Voice = "low" { c,1 }
}
