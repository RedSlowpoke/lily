\version "2.26.0"
% N7's lyric combine iterator where the 30 scores do not reach it (MS2
% step 38), with no melisma (step 39's): no syllable over a rest, a grace
% note's syllable waiting for its main note and one taken with
% includeGraceNotes, a voice made later than its lyrics, associatedVoice
% switched to a voice iterated before the lyrics (as one after them gives
% them no syllable, in LilyPond too), the voice's repeats forwarded to its
% lyrics, a voice moved by \change, two \addlyrics, and \lyricsto of a
% voice that never comes and of music without Lyrics. One score: the
% engine translates a file's scores last first (progress.md, step 38).
\score {
  <<
    \new Staff \new Voice = "one" \relative {
      c'4 r d \grace e16 f4 |
      \repeat volta 2 { g4 a b c } \alternative { { d2 e } { f1 } }
      g4 a b c
    }
    \new Lyrics \lyricsto "one" { a b c d e f g h i j k l m }
    \new Lyrics \with { includeGraceNotes = ##t } \lyricsto "one" {
      a b c d e f
    }
    \new Staff {
      s2 \new Voice = "late" \relative { e'4 f g a b c d e }
    }
    \new Staff \new Voice = "two" \relative { g'8 a b c d4 e f g a b c d }
    \new Lyrics \lyricsto "late" {
      la la la \set associatedVoice = "two" lo lo lo
    }
    \new Lyrics \lyricsto "nowhere" { x y }
    \new Staff = "up" \new Voice = "moving" \relative {
      c''4 d \change Staff = "down" e f | \change Staff = "up" g1
    }
    \new Lyrics \lyricsto "moving" { a b c d e }
    \new Staff = "down" { \clef bass s1 s1 }
    \new Staff \relative { c'4 d e f }
    \addlyrics { one two three four }
    \addlyrics { uno dos tres }
    \new Voice = "bare" \relative { c'4 d e f }
    \lyricsto "bare" { a b c d }
  >>
}
