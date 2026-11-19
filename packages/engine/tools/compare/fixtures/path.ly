\version "2.26.0"
% The layout path's fixture (MS3 step 25): a staff of skips whose page lays
% out once the path is in (step 35), before Y1's notation. Its \layout
% removes the engravers whose grobs need callbacks the path does not port:
% the clef's and the key's (clef.cc, key-signature-interface.cc), the
% ledger lines' (ledger-line-spanner.cc), the system start delimiter's
% (system-start-delimiter.cc) and the bar numbers', placed by
% side-position-interface (step 40). What is left is the staff symbol, the
% axis groups and their alignment, the paper columns, break alignment and
% spacing, and the bar lines and time signatures, which bar-line.scm and
% time-signature.scm draw. The skips' musical columns hold nothing, and
% none is live after line breaking: the line is its non-musical columns.
\score {
  \new Staff {
    \time 3/4 s2. | s4 s2 | s8 s4. s4 |
    \time 2/4 s2 | s4 s8 s8 | s2 \bar "|."
  }
  \layout {
    \context {
      \Staff
      \remove Clef_engraver
      \remove Key_engraver
      \remove Ledger_line_engraver
    }
    \context {
      \Score
      \remove System_start_delimiter_engraver
      \remove Bar_number_engraver
    }
  }
}
