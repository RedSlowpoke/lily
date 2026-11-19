\version "2.26.0"
% MS4 step 15's lyric grobs where the 30 scores do not reach them, laid
% out: syllables held back by slurs, a tie and \melisma, with extenders by
% __ and automatic ones; hyphens and extenders across a bar line and across
% a \break; lyricMelismaAlignment; \lyricmode spaces (_) that align the
% syllable before, and "some _ -- words"; stanza numbers by a string and by
% a markup; ignoreMelismata with extendersOverRests; \addlyrics; a voice
% that ends before its lyrics, leaving a hyphen unterminated; and lyrics
% that follow no voice. fixtures/lyrics.ly is N7's trace of the iterator.
\paper { ragged-right = ##t }
\score {
  <<
    \new Staff \new Voice = "one" \relative {
      c'4( d e) f |
      g2~ g4 a |
      b4 \melisma c d \melismaEnd e |
      f2 e2~ | \break
      e4 d( c) f |
      g4 r f e |
      d1 |
    }
    \new Lyrics \lyricsto "one" {
      \set stanza = "1."
      Ky -- ri __ e e -- lei __ son glo -- ri __ a -- men
      in ex -- cel -- sis De -- o
    }
    \new Lyrics \with { lyricMelismaAlignment = #RIGHT } \lyricsto "one" {
      \set stanza = \markup { \bold "2." }
      some _ -- words _ _ __ and more _ _ _ and -- so on
    }
    \new Lyrics \with { ignoreMelismata = ##t extendersOverRests = ##t }
    \lyricsto "one" {
      a b __ c d e f g h -- i j k l m -- n o p q r s t u
    }
    \new Staff \new Voice = "short" \relative { c''2( d) e1 }
    \new Lyrics \lyricsto "short" { long -- er __ than the voice -- }
    \new Staff \relative { e'4 f g a | b1 }
    \addlyrics { one -- two three __ four five }
    \new Lyrics \lyricmode { free4 ly2 -- ing4 }
  >>
}
