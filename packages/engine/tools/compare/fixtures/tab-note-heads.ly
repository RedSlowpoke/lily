\version "2.26.0"

% MS4 step 7a: Tab_note_heads_engraver and Tab_staff_symbol_engraver, the
% TabNoteHeads of determine-frets (grobs.test.mjs): frets found, string
% numbers in a chord and outside one, fingerings, minimumFret, a note no
% string reaches, a slur and a tie in a TabVoice; a bass's four strings
% with letters, string one at the bottom. The Staff has the string numbers
% of a note on its own only in the TabStaff (OPEN A45: a Voice's are not
% made).

music = \relative c' {
  c4 e g c
  <c,\2 e\1>4 <g\4 c\3>4 c4-\tag tab \3 e-\tag tab \2
  <c-1 e-3>2 <e-2 g-4 c-1>2
  c8( d) e4~ e g
  \set TabStaff.minimumFret = #5
  c,4 d e f
  \unset TabStaff.minimumFret
  c,,4 <c'' e>4 r2
}

\score {
  <<
    \new Staff { \clef "treble_8" \removeWithTag tab \music }
    \new TabStaff { \music }
    \new TabStaff \with {
      stringTunings = #bass-tuning
      tablatureFormat = #fret-letter-tablature-format
      stringOneTopmost = ##f
    } \relative c, { c4 e g c e,1 }
  >>
}
