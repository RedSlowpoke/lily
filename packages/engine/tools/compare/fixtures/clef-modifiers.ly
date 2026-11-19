\version "2.26.0"
% ClefModifier (MS3 step 42): the 8 or 15 below or above a transposed clef
% (Clef_engraver), aligned on its clef for the clef's glyph
% (ly:clef-modifier::calc-parent-alignment) and placed by its side
% position, each score one line, which the engine lays out whole. Where a
% modifier meets its clef's outline rather than the staff's padding (a G
% clef, a C clef off the staff), its skyline is the box of its extents in
% both: the engine's text skylines are boxes (W1's text packs hold no
% outlines), LilyPond's the digits' outlines.
% Clear of their clefs, on the staff's padding: below and above, F and C
% clefs, 8 and 15, parenthesized and bracketed.
\score { \relative {
  \clef "bass_8" c4 d e f | \clef "bass^15" c' d e f |
  \clef "alto_8" c d e f | \clef "tenor_8" c d e f | c1 \bar "|."
} }
\score { \relative {
  \clef "alto^(8)" c'4 d e f | \clef "bass^[15]" c,, d e f |
  \clef "varbaritone^8" c d e f | \clef "subbass_15" c d e f | c1 \bar "|."
} }
% Against their clefs: G clefs, and C clefs off the staff.
\score { \relative {
  \override Staff.ClefModifier.vertical-skylines = #ly:grob::simple-vertical-skylines-from-extents
  \clef "treble_8" c'4 d e f | \clef "treble^8" c'' d e f |
  \clef "treble_15" c,, d e f | \clef "treble^(15)" c d e f | c1 \bar "|."
} }
\score { \relative {
  \override Staff.ClefModifier.vertical-skylines = #ly:grob::simple-vertical-skylines-from-extents
  \clef "soprano_8" c'4 d e f | \clef "tenor^8" c d e f |
  \clef "french_8" c' d e f | \clef "baritone^8" c,, d e f | c1 \bar "|."
} }
% A clef forced again, a full-size change.
\score { \relative {
  \clef "bass_8" c4 d e f | \set Staff.forceClef = ##t c1 |
  \once \override Staff.Clef.full-size-change = ##t \clef "bass^8" c4 d e f |
  \clef "alto_8" c' d e f | c1 \bar "|."
} }
% clef-alignments set: F's no pair (centred), C's changed.
\score { \relative {
  \override Staff.ClefModifier.vertical-skylines = #ly:grob::simple-vertical-skylines-from-extents
  \override Staff.ClefModifier.clef-alignments = #'((F . 0.5) (C . (0.6 . -0.4)))
  \clef "bass_8" c4 d e f | \clef "alto_8" c' d e f |
  \clef "tenor^8" c d e f | \clef "bass^15" c, d e f | c1 \bar "|."
} }
