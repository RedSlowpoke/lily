\version "2.26.0"
% Script and ScriptColumn (MS3 step 47, for step 53): articulations above
% and below their notes (Script_engraver, script-interface.cc), stacked in
% a script column by their script-priority (Script_column_engraver,
% script-column.cc), each score one line.
% Scripts by their default direction, forced up and down, stacked two and
% three on a note, on a chord, and on a rest's fermata.
\score { \relative {
  c''4-. d-> e-^ f-- | g-+ a-! b-_ c\fermata |
  c,4^. d_> e^\accent f_\tenuto | g-.-> a->-- b-.-^-- c\turn |
  <c, e g>4-> <d f a>-. <e g b>_\marcato r\fermata \bar "|."
} }
% Two voices, the scripts on the stem's side of each; a script inside the
% staff (staccato's avoid-slur) and one outside it (\fermata's
% outside-staff-priority); a trill and a mordent.
\score { \relative {
  << { c''4-. d-> e\trill f\mordent } \\ { a,4-. b-> c-- d\fermata } >> |
  c4\staccatissimo d\espressivo e\upbow f\downbow \bar "|."
} }
