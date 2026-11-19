\version "2.26.0"
% MS4 step 17: tablature, fret diagrams and chord names. A melody on a
% Staff and a TabStaff, with string numbers (a lone note's for the
% TabStaff only, OPEN A45), a tie whose tab head is parenthesized after
% the \break, and a slur; FretBoards of the predefined diagrams and of the
% notes (with barres), chordChanges; a ChordNames line over a staff,
% chordChanges again; and a \score with a TabStaff in a markup (OPEN A47).

\include "predefined-guitar-fretboards.ly"

\paper { ragged-right = ##t }

melody = \relative c' {
  c4 e g-\tag tab \3 c | e,8( f) g4 <c, e\2>2~ | \break
  <c e\2>2 c'4-\tag tab \2 b |
}

harmonies = \chordmode { c1 a:m g:7 c }

<<
  \new ChordNames {
    \set chordChanges = ##t
    \harmonies
  }
  \new FretBoards {
    \set chordChanges = ##t
    \harmonies
  }
  \new FretBoards { <c e g c'>1 <a, e a c' e'>1 }
  \new Staff { \clef "treble_8" \removeWithTag tab \melody c'1 }
  \new TabStaff { \melody c'1 }
>>

\markup {
  \score {
    \new TabStaff { \relative c' { c4 d e f } }
    \layout { }
  }
}
