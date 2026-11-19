\version "2.26.0"

% MS5 step 6a: \partCombine's two parts on one staff, as part-combiner.scm
% splits them and Part_combine_iterator moves them between the Voices one,
% two, shared, solo and null: unisons and a2, solo passages of each part,
% chords, rests and multi-measure rests in one part and in both, a slur and
% a tie in one part, dynamics and a hairpin, \partCombineApart,
% \partCombineChords and \partCombineSoloI; then \partCombineUp, and the
% parts on channels of their own (midiChannelMapping = #'voice).

partOne = \relative c'' {
  c4\f d e f
  g2( f4) e
  R1
  c4 c c c
  e2~ e4 d
  \partCombineApart c4 d \partCombineAutomatic e f
  r2 g4\< a
  b1\!
}

partTwo = \relative c' {
  e4\p f g a
  g2 f4 e
  g4 a b c
  c4 c c c
  R1
  \partCombineChords a4 b c d
  r2 e4 f
  g1
}

\score {
  \new Staff \partCombine \partOne \partTwo
  \midi { \tempo 4 = 100 }
}

\score {
  \new Staff \partCombineUp \relative c'' {
    c4 d \partCombineSoloI e f | g1
  } \relative c'' {
    a4 b c d | g,1
  }
  \midi {
    \context {
      \Score
      midiChannelMapping = #'voice
    }
  }
}
