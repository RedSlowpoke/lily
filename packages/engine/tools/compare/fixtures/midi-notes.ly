\version "2.26.0"
% MS5 step 1's plain notes (07 §7's M2): two staves, one named and its
% voices named, the other not; rests, chords, a unison of two voices,
% grace notes at the start and after a note, a tuplet and a tie-less
% \tempo-less start, so each track's name, channel, notes and the first
% tempo are LilyPond's. A second score, under a title, is the file's
% -1.midi, its control track named after the title.
\score {
  <<
    \new Staff = "upper" <<
      \new Voice = "one" \relative {
        \voiceOne \grace { b16 d } e'4 <c e g> r8 g' \tuplet 3/2 { f8 e d } |
        c2 \grace f16 e4 r |
        <g, b d>1 |
      }
      \new Voice = "two" \relative {
        \voiceTwo c'2 e4 e |
        r4 c8 d e2 |
        g,1 |
      }
    >>
    \new Staff \relative {
      \clef bass c,4 r8 g' c,4 <c g'> |
      r2 c'4. b8 |
      c,1 |
    }
  >>
}
\score {
  \header { title = "Plain notes" }
  \relative { a'8 b c d e4 r | a,1 | }
}
