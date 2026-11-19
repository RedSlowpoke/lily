\version "2.26.0"
% RestCollision (MS3 step 41): the rests of several voices on one staff
% moved clear of each other and of the other voices' notes as
% rest-collision.cc places them (Rest_collision_engraver,
% ly:rest-collision::calc-positioning-done and
% ly:rest-collision::force-shift-callback-rest), each score one line,
% which the engine lays out whole.
% Rests against notes, up and down, high and low; a dotted rest; rests
% against a note begun before them; chords; whole and half rests.
\score { \relative {
  \cadenzaOn \autoBeamOff
  << { c''4 r4 e4 r2 } \\ { r4 g,4 r4 f2 } >> \bar "|"
  << { g'2 a4 } \\ { r4 r4 r4 } >> \bar "|"
  << { r4. c8 } \\ { e,4 r8 f8 } >> \bar "|"
  << { <c e g>4 r4 } \\ { r4 <a c e>4 } >> \bar "|"
  << { r1 } \\ { c,1 } >> \bar "|"
  << { a''2 } \\ { r2 } >> \bar "|."
} }
% Rests only: of equal and different lengths, whole rests, dotted rests,
% one voice's rest against another's note in the same column.
\score { \relative {
  \cadenzaOn \autoBeamOff
  << { r4 r2 r1 } \\ { r4 r2 r1 } >> \bar "|"
  << { r4 } \\ { r2 } >> \bar "|"
  << { r4. } \\ { r8 } >> \bar "|"
  << { r8 r8 } \\ { r4 } >> \bar "|."
} }
% Three voices of rests; rests without a direction, LilyPond's warning;
% rests of one direction only, its warning of too many colliding rests; a
% pitched rest, left in place; a larger minimum-distance; three voices of
% rests and notes, two rests moved up, the same warning.
\score { \new Staff \relative {
  \cadenzaOn \autoBeamOff
  << { r4 } \\ { r4 } \\ { r4 } >> \bar "|"
  << \new Voice { r4 } \new Voice { r4 } >> \bar "|"
  << \new Voice { \voiceOne r4 } \new Voice { \voiceOne r2 } >> \bar "|"
  << { a'4\rest c4 } \\ { r4 r4 } >> \bar "|"
  \override Staff.RestCollision.minimum-distance = 2
  << { c4 r4 } \\ { r4 a4 } >> \bar "|"
  \revert Staff.RestCollision.minimum-distance
  << { c4 } \\ { r4 } \\ { e4 } >> \bar "|"
  << { r4 } \\ { c4 } \\ { r4 } >> \bar "|."
} }
