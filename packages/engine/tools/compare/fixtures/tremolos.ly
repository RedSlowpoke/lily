\version "2.26.0"
% StemTremolo and chord tremolos (MS4 step 5): stem tremolos (Stem_engraver,
% stem-tremolo.cc) on unbeamed stems up and down, with flags, on a beamed
% stem and on whole and half notes; chord tremolos (Chord_tremolo_engraver)
% between quarters, halves and whole notes; and beams kept clear of what
% they cover (Beam_collision_engraver): a clef and a key change inside a
% beam, and another voice's notes under one; each score one line.
% Stem tremolos.
\score { \relative {
  c''4:16 c,:32 c''8:32 c,,:64 b''4:8 | g1:16 |
  a,2:32 \stemDown e''2:16 \stemNeutral |
  c,8:16[ d e:32 f] g4:64 a:16 \bar "|."
} }
% Chord tremolos.
\score { \relative {
  \repeat tremolo 4 { c'16 e } \repeat tremolo 8 { g32 b }
  \repeat tremolo 4 { a8 f' } |
  \repeat tremolo 8 { c,16 g' } \repeat tremolo 16 { c,32 e } |
  \repeat tremolo 4 { <c e>16 <g' b> } r2 \bar "|."
} }
% Beams over a clef, a key change and another voice.
\score { \relative {
  c''8[ \clef bass d,, \key g \major e f] \clef treble g''[ a b c] |
  << { c8[ g' c, g'] f[ e d c] } \\ { e,4 g' f, a' } >> \bar "|."
} }
