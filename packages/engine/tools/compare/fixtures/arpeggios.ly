\version "2.26.0"
% Arpeggio, its chord bracket and chord slur, and the span arpeggio (MS3
% step 47, for step 54): arpeggios on chords (Arpeggio_engraver,
% arpeggio.cc), up and down with their arrows, as a bracket and as a
% parenthesis, and one arpeggio across a group's two staves
% (Span_arpeggio_engraver), each score one line. No PianoStaff: a brace's
% font is in no preloaded pack.
\score { \relative {
  <c' e g c>4\arpeggio
  \arpeggioArrowUp <c e g c>4\arpeggio
  \arpeggioArrowDown <c e g c>4\arpeggio
  \arpeggioNormal <d f a d>4\arpeggio |
  \arpeggioBracket <c e g c>4\arpeggio
  \arpeggioParenthesis <c e g c>4\arpeggio
  \arpeggioParenthesisDashed <c e g>4\arpeggio
  \arpeggioNormal << { <e' g c>4\arpeggio } \\ { <c, e>4\arpeggio } >> \bar "|."
} }
\score {
  \new StaffGroup \with { \consists "Span_arpeggio_engraver" connectArpeggios = ##t } <<
    \new Staff \relative { <c'' e g>4\arpeggio <d f a>2\arpeggio <c e g>4 }
    \new Staff \relative { \clef bass <c e g>4\arpeggio <b d f>2\arpeggio <c e g>4\arpeggio }
  >>
}
