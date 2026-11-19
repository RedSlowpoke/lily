\version "2.26.0"
% Tie, TieColumn, LaissezVibrerTie and LaissezVibrerTieColumn (MS4 step 9,
% for steps 10 and 11): single ties up and down by their stems, on lines
% and in spaces, inside and outside the staff, across a bar and a line
% break; chords' tie columns, seconds, a chord tied in part, dotted chords
% and chords with accidentals; \tieUp, \tieDown and the event's own
% direction; a tie over a beam; ties to and from grace notes; ties set by
% hand (tie-configuration, staff-position); laissez vibrer ties, alone and
% on chords; ties waiting for their notes; two voices tied; then
% Tie_engraver's warnings.
% Single ties: by the stems, on lines and in spaces, outside the staff,
% across a bar, whole notes and short ones.
\score { \relative {
  c''4~ c a~ a | f~ f d'~ d | g,2~ g | a'4~ a c,,~ c |
  e'1~ | e | b8~ b d~ d f~ f a,~ a \bar "|."
} }
% Chords: tie columns of two to four ties, seconds, a chord tied in part,
% dotted chords and chords with accidentals after the tie.
\score { \relative {
  <c' e g>2~ <c e g> | <f a>~ <f a> | <g a>~ <g a> | <c, d e>~ <c d e> |
  <e g c e>~ <e g c e> | <c e g>~ g' | <d f a>4.~ <d f a>8 <e g b>4.~ <e g b>8 |
  <fis a d>2~ <fis a d> | <b dis fis>1~ | <b dis fis> \bar "|."
} }
% Directions forced: \tieUp, \tieDown, the event's ^~ and _~, a chord.
\score { \relative {
  \tieUp c''4~ c e,~ e | \tieDown a~ a f'~ f | \tieNeutral g^~ g g_~ g |
  \tieDown <c, e>2~ <c e> | \tieNeutral <c e g>^~ <c e g> \bar "|."
} }
% A tie over a beam, and ties into and out of beamed groups.
\score { \relative {
  c''8[~ c d e]~ e[ f~ f g] | a,8[ b c d~] d4 r | e16[ f g a~] a4~ a2 \bar "|."
} }
% Ties to and from grace notes.
\score { \relative {
  c''2~ \grace { c16 } d2 | \grace { e16~ } e2 f~ | \grace { f16 } g1 \bar "|."
} }
% A tie broken across a line break, and a chord's.
\score { \relative {
  c''1~ | \break c | <e, g>1~ | \break <e g> \bar "|."
} }
% Ties set by hand: tie-configuration of a column, a tie's staff-position
% (an exact one and an inexact one), and a tie's direction.
\score { \relative {
  \once \override TieColumn.tie-configuration = #'((-1 . -1) (5 . 1))
  <c' g'>2~ <c g'> |
  \once \override Tie.staff-position = #3
  e'2~ e |
  \once \override Tie.staff-position = #-0.5
  c2~ c |
  \once \override Tie.direction = #DOWN
  <c e g>2~ <c e g> \bar "|."
} }
% Laissez vibrer ties, on single notes and chords, one direction forced;
% and a chord with one note's laissez vibrer.
\score { \relative {
  c''4\laissezVibrer r e\laissezVibrer r | <c e g>2\laissezVibrer r |
  g4^\laissezVibrer r <c e\laissezVibrer>2 \bar "|."
} }
% Ties waiting for their notes (tieWaitForNote), and two voices tied.
\score { \relative {
  \set tieWaitForNote = ##t
  c''8~ e~ g~ <c, e g>8~ <c e g>2 |
  \unset tieWaitForNote
  << { e'2~ e4 d~ | d1 } \\ { c,2~ c4 b~ | b1 } >> \bar "|."
} }
% Tie_engraver's warnings: a tie to no note of its pitch, and an
% unterminated tie at the end of the music.
\score { \relative {
  c''4~ d e f | g1~
} }
