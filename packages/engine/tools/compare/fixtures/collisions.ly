\version "2.26.0"
% NoteCollision (MS3 step 41): the note columns of several voices on one
% staff shifted apart, merged or meshed as note-collision.cc places them
% (Collision_engraver, ly:note-collision-interface::calc-positioning-done),
% each score one line, which the engine lays out whole.
% Too far apart; a second, the up-stem above and below; unisons of
% quarters, halves and wholes; a quarter on a half, unmerged; merged
% differently headed and dotted; an eighth on a half.
\score { \relative {
  \cadenzaOn
  << { g'4 c d c2 c1 c4 } \\ { e,4 b' e c2 c1 c2 } >> \bar "|"
  \mergeDifferentlyHeadedOn \mergeDifferentlyDottedOn
  << { c4 c4. c8 c8 } \\ { c2 c8 c4. c2 } >> \bar "|."
} }
% Dotted collisions: dots right, dots left, a dotted second; chords that
% touch, that mesh, that collide fully.
\score { \relative {
  \cadenzaOn
  << { c''4. d4 c4. e4. } \\ { c4 d4. b4. f4 } >> \bar "|"
  << { <e g>4 <c e> <c e g> <b d> } \\ { <a, c>4 <c e> <b d f> <c e> } >> \bar "|."
} }
% Four voices; a third voice shifted as the first; force-hshift; an
% ignored column; fa heads; whole notes a second apart, a dotted whole on
% a whole.
\score { \relative {
  \cadenzaOn
  << { e''4 f } \\ { c4 d } \\ { a'4 g } \\ { f,4 e } >> \bar "|"
  << { e'4 f } \\ { c4 d } \\ { \voiceOne g4 a } >> \bar "|"
  << { e4 \once \override NoteColumn.force-hshift = 1.5 f } \\ { c4 d } >> \bar "|"
  << { c4 \once \override NoteColumn.ignore-collision = ##t b } \\ { b4 c } >> \bar "|"
  << { \override NoteHead.style = #'fa c4 } \\ { \override NoteHead.style = #'fa c4 } >> \bar "|"
  << { d1 c1. } \\ { c1 c1 } >> \bar "|."
} }
% Two up-stem voices without a shift, LilyPond's warning at each column
% after the first; equal shifts; dotted up-stem voices clearing the
% shifted one's stem; meshed chords with dots on the left.
\score { \new Staff \relative {
  \cadenzaOn
  << \new Voice { \stemUp e''4 f } \new Voice { \voiceTwo c4 d }
     \new Voice { \stemUp g4 a } >> \bar "|"
  << { \shiftOn e4 f } \\ { c4 d } \\ { \shiftOn g4 a } >> \bar "|"
  << { a4. g4. } \\ { c,4 d } \\ { f4. e4. } >> \bar "|"
  << { <c e>4 <d f>4. } \\ { <e, g>4. <f a>4 } >> \bar "|."
} }
% Chords that collide fully, the down-stem dotted (stem to stem); a close
% and a distant half collision alone; meshed chords, dotted and not;
% merges wiping the down-stem dots of equal ones, the head with fewer
% dots, the shorter head, an eighth on a half; a shifted voice crossing
% the down-stem voice.
\score { \new Staff {
  \cadenzaOn
  << { <c'' e''>4 } \\ { <c'' e''>4. } >> \bar "|"
  << { <b' f''>4 } \\ { <a' d''>4 } >> \bar "|"
  << { <b' f''>4 } \\ { <e' c''>4 } >> \bar "|"
  << { <b' f''>4 } \\ { <g' d''>4 } >> \bar "|"
  << { <b' f''>4. } \\ { <g' d''>4. } >> \bar "|"
  << { c''4. } \\ { c''4. } >> \bar "|"
  \mergeDifferentlyHeadedOn \mergeDifferentlyDottedOn
  << { c''4. } \\ { c''4 } >> \bar "|"
  << { c''4 } \\ { c''8 } >> \bar "|"
  << { c''8 } \\ { c''4 } >> \bar "|"
  << { c''8 } \\ { c''2 } >> \bar "|"
  \mergeDifferentlyHeadedOff \mergeDifferentlyDottedOff
  << { \shiftOn e''4 } \\ { c''4 } \\ { \shiftOn b'4 } >> \bar "|."
} }
