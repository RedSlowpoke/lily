\version "2.26.0"
% T23's incipits (aucun-snippet.ly:19–99) as the instrument names of a
% score with no tie, beam or lyrics (MS3 step 43): \scores in markups,
% each translated with its \layout and laid out when its name prints
% (score-lines, ly:score-embedded-format). trace.ly records them by their
% Score's translator alone, as their \layout is copied from $defaultlayout
% before its book handler adds the recording engraver: their S lines
% after the score's X lines. T23 itself is not laid out before MS4 (its
% TieColumn's before-line-breaking), so its incipits are not translated.
incipitGlobal = {
  \override NoteHead.style = #'mensural
  \override Accidental.style = #'mensural
  \override KeySignature.style = #'mensural
  \override Rest.style = #'mensural
  \cadenzaOn
  \override Score.Clef.extra-offset = #'(-0.0 . 0.5)
  \override Score.Clef.font-size = #3
  \clef "vaticana-do1"
}

incipitTriplum = \markup {
  \score {
    {
      \set Staff.instrumentName = "Triplum"
      \incipitGlobal
      s1.
    }
    \layout {
      \context {
        \Voice
        \remove Ligature_bracket_engraver
        \consists Mensural_ligature_engraver
      }
      \context {
        \Staff
        \remove Time_signature_engraver
      }
      line-width = 5\mm
    }
  }
}

incipitMotetus = \markup {
  \score {
    {
      \set Staff.instrumentName = "Motetus"
      \incipitGlobal
      s1.
    }
    \layout {
      \context {
        \Voice
        \remove Ligature_bracket_engraver
        \consists Mensural_ligature_engraver
      }
      \context {
        \Staff
        \remove Time_signature_engraver
      }
      line-width = 5\mm
    }
  }
}

incipitTenor = \markup {
  \score {
    {
      \set Staff.instrumentName = "Tenor"
      \incipitGlobal
      s1.*2
    }
    \layout {
      \context {
        \Voice
        \remove Ligature_bracket_engraver
        \consists Mensural_ligature_engraver
      }
      \context {
        \Staff
        \remove Time_signature_engraver
      }
      line-width = 5\mm
    }
  }
}

\score {
  \new StaffGroup <<
    \new Staff { \set Staff.instrumentName = \incipitTriplum \time 3/4 c''2. d''2. }
    \new Staff { \set Staff.instrumentName = \incipitMotetus \time 3/4 e'2. f'2. }
    \new Staff { \set Staff.instrumentName = \incipitTenor \time 3/4 \clef bass c2. d2. }
  >>
  \layout { }
}
