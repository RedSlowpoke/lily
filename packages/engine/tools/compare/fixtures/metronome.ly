\version "2.26.0"
% MetronomeMark (MS3 step 47, for step 54): tempo marks (Metronome_mark_
% engraver) with a beat and a count, with a text, a text alone, a range
% and a dotted beat, each placed above its staff at its column.
\score { \relative {
  \tempo 4 = 120 c''4 d e f |
  \tempo "Allegro" 4 = 132 g4 a b c |
  \tempo "Andante" c,4 d e f |
  \tempo 4 = 60 - 72 g4 a b c |
  \tempo 4. = 80 c2. r4 \bar "|."
} }
