\version "2.26.0"

% Midi_walker's two cases that no score of the 42 reaches (MS5 step 1's,
% kept for step 8's gate): a grace note whose tick (9/40 of its grace
% time) comes before that of the note just ahead of it in moment order,
% which LilyPond writes with no delta after "Going back in MIDI time.";
% and notes shorter than a tick but not of no length, which LilyPond
% starts and stops at one tick, a note of no length at all not written.

\score {
  \new Staff \relative c' {
    c4*24/25 d4*1/25 \grace e16 f4 g2 |
    c,4*1/500 d4*499/500 e4*1/2000 r4*1999/2000 <f a>4*1/600 r4*599/600 g4 |
    \grace { a16 b } c4*24/25 d4*1/25 \grace { e16 f } g2. |
  }
  \midi { }
}
