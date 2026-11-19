\version "2.26.0"
% Y1's grobs that none of the 10 reaches (MS3 step 25), for steps 41–43:
% a Flag of each unbeamed eighth and sixteenth (no beam: beams are MS4's),
% a NoteCollision of two voices' notes and a RestCollision of their rests,
% MultiMeasureRests, the compressed ones with their MultiMeasureRestNumber,
% one with its MultiMeasureRestText and one with its MultiMeasureRestScript,
% a ClefModifier of each octavated clef, an InstrumentName of each named
% staff, and a StaffGroup whose SpanBars join its two staves' bar lines,
% with the SpanBarStubs of the lyrics between them (Span_bar_stub_engraver
% makes them in a context without bar lines, which Dynamics is not).
% Then VerticalAxisGroup's suicide (MS3 step 28), each staff removed when
% empty even on the first system (\RemoveAllEmptyStaves): in the first
% PianoStaff, whose layers are unset, a staff of rests kept alive by the
% other (keep-alive-with); in the second, a staff of notes of layer 2
% killed by the one of layer 1 (make-dead-when); and a staff of skips
% that no other keeps, removed. The PianoStaffs have no SpanBar.
<<
\new StaffGroup <<
  \new Staff \with { instrumentName = "Vln." } \relative {
    \clef "treble^8" \autoBeamOff
    c''8 d16 e f4 << { g4 a } \\ { e4 f } >> |
    << { r4 b4 } \\ { r4 g4 } >> c2 |
    \compressEmptyMeasures R1*3^"tacet" |
    R1\fermata |
    c,4 d e f \bar "|."
  }
  \new Lyrics \lyricmode { la1 la1 la1*3 la1 la1 }
  \new Staff \with { instrumentName = "Vc." } \relative {
    \clef "bass_8"
    c4 d e f | g2 a |
    \compressEmptyMeasures R1*3 |
    R1 |
    c,4 d e c \bar "|."
  }
>>
\new PianoStaff \with { \remove Span_bar_engraver } <<
  \new Staff \with { \RemoveAllEmptyStaves } { c'1 d'1 }
  \new Staff \with { \RemoveAllEmptyStaves } { \clef bass r1 r1 }
>>
\new PianoStaff \with { \remove Span_bar_engraver } <<
  \new Staff \with {
    \RemoveAllEmptyStaves
    \override VerticalAxisGroup.remove-layer = 1
  } { e'1 f'1 }
  \new Staff \with {
    \RemoveAllEmptyStaves
    \override VerticalAxisGroup.remove-layer = 2
  } { g'1 a'1 }
>>
\new Staff \with { \RemoveAllEmptyStaves } { s1 s1 }
>>
