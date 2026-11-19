\version "2.26.0"
% MS5 step 2's dynamics (07 §7's M3): each note's velocity from
% Dynamic_performer's spans. Absolute dynamics from ppppp to sf and one
% the volume function does not know; hairpins to a dynamic, from one,
% against the dynamic they end at, a crescendo turning into a
% decrescendo with and without a dynamic after it, text crescendi, and a
% hairpin with no end; midiMinimumVolume and midiMaximumVolume set
% midway; two instruments through the equalizer (instrumentName, so no
% program change), a staff of two voices, a piano's staff not equalized,
% and accents' midi-extra-velocity.
sfzp = #(make-dynamic-script "sfzp")
\score {
  <<
    \new Staff \with { instrumentName = "violin" } \relative {
      c'4\p\< d e f | g\f\> f e d | c\pp\< d e f\! | g\mp\< a b c\> |
      b a g f\! | e\ff\> d c d\< | e1\pp | c4\sf r c\fffff c\ppppp |
      c\p\< d e f | g\pp a b c | c,\mf\< d e f\> | g a b c |
    }
    \new Staff \with { instrumentName = "flute" } <<
      \new Voice \relative {
        \voiceOne e''4\mf\accent d\marcato c\cresc b | a\f g\decresc f e |
        \set Staff.midiMinimumVolume = #0.3
        \set Staff.midiMaximumVolume = #0.9
        d\p\< e f g | a\> g f e\! | d1\p\< | c4 \dim d e f |
        g1\! | g4 g\accent g g | a\mp b c d | e1 | d | c |
      }
      \new Voice \relative {
        \voiceTwo c''2\pp\< e | f1\mf | c2 e | f1 |
        c2\< e | f1\! | c2 e | f1 | f\> | f | f\! | f |
      }
    >>
    \new Staff \relative {
      \clef bass c4 d\< e f | g\sfzp f e d\> |
      c1 | c4\mf r c\< r | c1 | c4 c c c\! | c1 | c | c | c | c | c |
    }
  >>
}
