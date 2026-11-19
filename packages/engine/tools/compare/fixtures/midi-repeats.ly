\version "2.26.0"

% MS5 step 6: music changed before it is performed. \unfoldRepeats over
% every repeat type (volta with alternatives in both syntaxes and fewer
% alternatives than times, percent of a beat and of bars, tremolo of a
% note and of two, segno with \alternative and \fine, unfold with
% \volta's selective music, a repeat in a repeat); \unfoldRepeats of
% chosen types; repeats played folded; and articulate.ly's \articulate
% over staccati, slurs, ornaments, a tremolo and a repeat; \fine in a
% folded repeat, before music and in an unfolded segno repeat.

\include "articulate.ly"

\score {
  \unfoldRepeats \new Staff \relative c' {
    \tempo 4 = 96
    \repeat volta 2 { c4 d e f }
    \alternative { { g2 g } { a2 a } }
    \repeat volta 3 {
      b4 c d e
      \alternative {
        \volta 1,2 { f2 f }
        \volta 3 { g1 }
      }
    }
    \repeat volta 3 { c,4 d e f }
    \alternative { { g1 } { a1 } }
    \repeat percent 4 { c8 d }
    \repeat percent 2 { e4 f g a | b2 c }
    \repeat tremolo 8 c16
    \repeat tremolo 4 { d16 e }
    \repeat segno 2 {
      f4 g a b
      \alternative {
        \volta 1 { c2 c }
        \volta 2 \volta #'() { \section \fine }
      }
    }
    \repeat unfold 3 { d4 \volta 2 e \volta 3 { f g } }
    \repeat volta 2 { \repeat unfold 2 { a8 b } c2 }
    c1
  }
  \midi { }
}

\score {
  <<
    \new Staff \unfoldRepeats #'(percent tremolo) \relative c'' {
      \repeat volta 2 { c4 d e f }
      \repeat percent 2 { g4 a }
      \repeat tremolo 4 { b16 c }
      d1
    }
    \new Staff \relative c' {
      \repeat volta 2 { c4 d e f }
      \alternative { { g1 } { a1 } }
      \repeat percent 2 { g4 a }
      \repeat tremolo 4 { b16 c }
      \repeat unfold 2 { d4 e }
      f1
    }
  >>
  \midi { }
}

\score {
  \articulate \new Staff \relative c'' {
    \tempo 4 = 72
    c4-. d-. e-- f->
    g8( a b c) d4-> \staccatissimo c
    b4\trill a\turn g\mordent f\prall
    e4\fermata d8 c \grace { d16 } c4 r
    \repeat volta 2 { c4( d) e-. f }
    \alternative { { g2 g } { a2. r4 } }
    \repeat tremolo 8 b16
    c1\fermata
  }
  \midi { }
}

\score {
  \new Staff \relative c' {
    \repeat volta 2 { c4 d e f \volta 2 \fine }
    g4 \fine a b c
    \unfoldRepeats \repeat segno 2 {
      d4 e f g
      \alternative {
        \volta 1 { a1 }
        \volta 2 { b2 \fine }
      }
    }
    c1
  }
  \midi { }
}
