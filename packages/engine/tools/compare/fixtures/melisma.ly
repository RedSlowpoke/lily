\version "2.26.0"
% N7's melismas where the 30 scores do not reach them (MS2 step 39): a
% syllable held back by a slur, slurs of two ids, a slur in a chord, a
% phrasing slur that holds none, ties of a chord and enharmonic, a tie to
% another pitch (LilyPond's "unterminated tie" holds the next note too) and
% with tieWaitForNote, manual beams with autoBeaming off and on, a grace
% beam, \melisma and \melismaEnd, and the same lyrics with
% ignoreMelismata and with includeGraceNotes. In \layout, traced, and in
% \midi, whose performers set the properties the lyrics read there.
\score {
  <<
    \new Staff \new Voice = "one" \relative {
      c'4( d) e f |
      g4\=1( a\=2( b\=1) c\=2) |
      <c, e>4( <d f>) <e g>\( f\) |
      g2~ g4 a |
      <c e>2~ <c e>4 b |
      cis2~ des4 c |
      d4~ e f g |
      \set tieWaitForNote = ##t
      a4~ b a c | \unset tieWaitForNote
      \set autoBeaming = ##f
      d8[ e f] g a[ b] c4 |
      \grace { b16[ a] } g2 f4 e |
      \unset autoBeaming
      d8[ e] f4 g a |
      b4 \melisma c d \melismaEnd e |
      f1
    }
    \new Lyrics \lyricsto "one" {
      a b c d e f g h i j k l m n o p q r s t u v w x y z
      aa bb cc dd ee ff gg hh ii jj kk ll mm nn oo pp
    }
    \new Lyrics \with { ignoreMelismata = ##t } \lyricsto "one" {
      a b c d e f g h i j k l m n o p q r s t u v w x y z
      aa bb cc dd ee ff gg hh ii jj kk ll mm nn oo pp qq rr ss tt
    }
    \new Lyrics \with { includeGraceNotes = ##t } \lyricsto "one" {
      a b c d e f g h i j k l m n o p q r s t u v w x y z
      aa bb cc dd ee ff gg hh ii jj kk ll mm nn oo pp
    }
  >>
  \layout { }
  \midi { }
}
