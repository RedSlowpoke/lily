\version "2.26.0"
% Beam (MS4 step 2, for steps 3 and 4): manual beams (Beam_engraver), the
% automatic ones (Auto_beam_engraver) by beatStructure and beamExceptions,
% grace beams of both kinds, \noBeam, beams over a rest and across a bar,
% subdivided beams, knees and chords, each score one line; then the beam
% engraver's three warnings.
% Manual beams of eighths, sixteenths and thirty-seconds, one over a rest,
% one across a bar line; forced stem directions.
\score { \relative {
  c'8[ d e f] g[ a b c] |
  c16[ b a g] f8[ r e d] c32[ d e f g a b c] |
  e,8[ f g a b c d e |
  f e] d4 \stemUp c8[ g'] \stemDown a[ e] \stemNeutral \bar "|."
} }
% Automatic beams in 4/4 by its beatStructure; \noBeam, \autoBeamOff and
% on; a rest ending a beam.
\score { \relative {
  c''8 d e f g a b c | c16 b a g f e d c c8 d\noBeam e f |
  b8 r a g f e d c | \autoBeamOff c8 d e f \autoBeamOn g a b c \bar "|."
} }
% Automatic beams in 3/4 and 6/8 by their beamExceptions.
\score { \relative {
  \time 3/4 c''8 d e f g a | c16 b a g f e d c b a g f |
  \time 6/8 a8 g f e d c | b16 c d e f g a8 b c \bar "|."
} }
% beamExceptions emptied and beatStructure set by hand; subdivided beams.
\score { \relative {
  \set Timing.beamExceptions = #'()
  \set Timing.beatStructure = #'(2 2)
  c''8 d e f g a b c |
  \set subdivideBeams = ##t
  c32 d e f g a b c c16 d e f c32 d e f g a b c c8 d \bar "|."
} }
% Grace beams: manual (Grace_beam_engraver) and automatic
% (Grace_auto_beam_engraver), and a grace note inside a main beam.
\score { \relative {
  \grace { c''16[ d] } e4 \grace { f16 g a } b4 c8[ \grace { d16 } c8]
  \grace e8 d4 | \grace { e16 f } g2 c,8 d e f \bar "|."
} }
% Knees and chords under beams.
\score { \relative {
  c'8[ c''] c,,[ c''] <c, e g>8[ <d f a>] <e g c>[ <f a d>] |
  c8[ c''' c,,, c'''] g,,16[ c e g] g8[ e c a] \bar "|."
} }
% The beam engraver's warnings: a beam started inside one, a stem too long
% for its beam, and a beam left unterminated, started on a skip.
\score { \relative {
  c''8[ d[ e f] c8[ d4 e8] | f2 s2[
} }
