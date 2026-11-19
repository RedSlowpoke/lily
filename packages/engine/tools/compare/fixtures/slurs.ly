\version "2.26.0"
% Slur and PhrasingSlur (MS4 step 7, for steps 8 and 11): slurs up and down,
% over beams and stems, across a bar line, with scripts inside and out,
% fingerings, text scripts, dots and accidentals they encompass, a
% phrasing slur over two slurs, slurs to and from grace notes, slurs on
% notes of a chord, double slurs, a slur broken across a \break; then the
% slur engravers' warnings.
% Slurs up and down, by the stems and forced; over a beam; across a bar.
\score { \relative {
  c''4( d) e( f) | g,( a b c) | \slurUp e8( d c b) \slurDown a( g f e) |
  \slurNeutral d8[( e f g]) a( b c4 | d2) c \bar "|."
} }
% Scripts that the slur encompasses and that avoid it outside, fingerings,
% a text script, dotted notes and accidentals under slurs.
\score { \relative {
  c''4-.( d-> e-- f-^) | g4\fermata( f-1 e-2 d-3) |
  c4.( d8 e4^"dolce" fis) | g4-+( a\turn bes2) \bar "|."
} }
% A phrasing slur over two slurs; phrasing slurs up and down.
\score { \relative {
  c''4\( d( e) f( | g) a b c\) | \phrasingSlurUp a\( g f e\) |
  \phrasingSlurDown d\( e( f) g\) \bar "|."
} }
% Slurs to and from grace notes, and in grace time.
\score { \relative {
  \grace { c''16( } d4) e \grace { f16( g } a4) b |
  c4( \grace { d16) } c4 \grace { b16( a } g2) \bar "|."
} }
% Slurs on notes of chords, and with spanner ids; double slurs.
\score { \relative {
  <c' e( g>4 <d f) a> <e g c>( <f a d>) |
  c'4\=1( d\=2( e\=2) f\=1) |
  \set doubleSlurs = ##t <c e>4( <d f> <e g>) r \bar "|."
} }
% A slur broken across a line break.
\score { \relative {
  c''4( d e f | \break g a b c) \bar "|."
} }
% The slur engravers' warnings: a slur started inside one of its id, a
% stop with no slur, and slurs left unterminated.
\score { \relative {
  c''4( d( e) f) | g4) a\) b c( | d\( e f g
} }
