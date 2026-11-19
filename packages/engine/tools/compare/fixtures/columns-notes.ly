\version "2.26.0"
% The columns' fixture of notes (MS3 step 48, OPEN A4), for the probe of
% the paper columns' skylines that fixtures/columns.ly's note names began:
% two staves whose non-musical columns hold clefs, keys, time signatures
% and bar lines, the changes of clef and key among them, and whose musical
% columns hold note heads with accidentals, dots, stems, flags, ledger
% lines, chords and rests, unbeamed: the engine makes no beams before MS4,
% and a beam kills its stems' flags and lengthens them, which changes the
% columns' skylines and the neighbors' pure heights a time signature's
% extra-spacing-height reads. The Vertical_align_engraver is removed, as
% columns.ly removes it: the probe reads the skylines when the Score is
% finalized, before its items are broken, and with the alignment LilyPond
% 2.26.0 then fails an assertion and aborts.
\layout {
  \context {
    \Score
    \remove Vertical_align_engraver
  }
}
\score {
  <<
    \new Staff {
      \autoBeamOff
      \clef treble \key d \major \time 3/4
      c''4 fis'8. g'16 r4 | <e' gis' b'>2 a''8 r |
      \clef bass \key bes \major d,4 r2 | f2. \bar "|."
    }
    \new Staff {
      \autoBeamOff
      \clef bass \key d \major \time 3/4
      d2 a,4 | b,4. cis8 d4 |
      \key bes \major es2 r4 | bes,,2.
    }
  >>
}
