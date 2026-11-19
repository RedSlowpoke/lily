\version "2.26.0"

% MS5 step 4's performers: Mark_performer's markers (rehearsal marks by
% default, by number and as text and markup, segno and coda marks, a
% section label), Tie_performer's ties (across a bar, in a chord, a tie
% of a chord, enharmonic, one that ends nowhere, tieWaitForNote, a tied
% note before grace notes and before a breath), Note_performer's breaths
% and Lyric_performer's syllables (melismata under slurs and ties,
% extenders, hyphens, a tilde, a markup, a skip, words that are not
% ASCII), sung to a Voice and to a NullVoice.

\score {
  <<
    \new Staff \relative c'' {
      \tempo 4 = 100
      \mark \default
      c2~ c8 d e4~
      e4 \breathe d2 c4(
      b4) a~ a8 g16 f e4 \breathe
      \mark \markup \bold "Bridge"
      <c e g>2~ <c e g>4 <d f>4
      <e g>2~ <e a>4 r
      \segnoMark \default
      cis'2~ des4 d~
      e2 f4 \breathe r4
      \codaMark \default
      \set tieWaitForNote = ##t
      g8~ a~ g a g2
      \unset tieWaitForNote
      \sectionLabel "Coda"
      f2~ \grace { e16 d } f4 e
      \mark 7
      d4~ \breathe d4 c2~
      \mark "Fine"
      c1
    }
    \addlyrics {
      Ky -- ri -- e e -- lei __ son,
      a~b \markup \italic "ich" _ grü -- ße
      _ dich, Ér -- de __ und See,
      la la la la
    }
    \new NullVoice = "null" \relative c' {
      c4 d e~ e f2( g4) a b1~ b2 c2
    }
    \new Lyrics \lyricsto "null" {
      eins zwei drei __ vier fünf __ sechs sie -- ben
    }
  >>
  \midi { }
}

\score {
  \new Staff \relative c' {
    \mark \default
    c4~ c4. d8 \breathe e4
    \mark \default
    <c e>4~ <c e>2 r4
    \mark \default
  }
  \midi { }
}
