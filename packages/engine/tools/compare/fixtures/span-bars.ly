\version "2.26.0"
% SpanBar and SpanBarStub (MS3 step 42): the bar lines of a group's staves
% joined by a span bar (Span_bar_engraver), each bar line widened towards
% it by its has-span-bar (OPEN A11), and a stub in each context between
% them without a bar line (Span_bar_stub_engraver), each score one line,
% which the engine lays out whole. No Lyrics: the engine's Lyrics line
% dies for want of LyricText, and its stubs with it.
% A group in a group: the inner group's span bars over repeat bars, then
% a staff without bar lines between two, its stub kept. (No PianoStaff: a
% brace's print is not the engine's yet, and leaves the page unwritten.)
\score { \new StaffGroup <<
  \new StaffGroup <<
    \new Staff \relative { c''4 d e f \bar ".|:" g1 \bar ":|.|:" a2 g \bar ":|." f1 \bar "|." }
    \new Staff \relative { c'4 d e f | g1 | a2 g | f1 }
  >>
  \new Staff \with { \remove Bar_engraver } \relative { c'4 d e f | g1 | a2 g | f1 }
  \new Staff \relative { \clef bass c4 d e f | g1 \bar "||" a2 g | f1 }
>> }
% A staff that allows no span bar below it: the stub below it killed.
\score { \new StaffGroup <<
  \new Staff \with { \override BarLine.allow-span-bar = ##f } \relative { c''4 d e f | g1 | a2 g | f1 }
  \new Staff \with { \remove Bar_engraver } \relative { c'4 d e f | g1 | a2 g | f1 }
  \new Staff \relative { \clef bass c4 d e f | g1 | a2 g | f1 \bar "|." }
>> }
% Staves out of their order (alignAboveContext), sorted; of other spaces
% and lines. The stubs' range is read from the span bar's elements in the
% order they were acknowledged, as LilyPond's: the staff without bar
% lines, above the first one acknowledged, gets none.
\score { \new StaffGroup <<
  \new Staff = "low" \with { \override StaffSymbol.line-count = 4 } \relative { \clef bass c4 d e f | g1 | a2 g | f1 \bar "|." }
  \new Staff \with { alignAboveContext = "low" \override StaffSymbol.staff-space = #0.8 } \relative { c''4 d e f | g1 | a2 g | f1 }
  \new Staff \with { alignAboveContext = "low" \remove Bar_engraver } \relative { c'4 d e f | g1 | a2 g | f1 }
  \new Staff \with { alignAboveContext = "low" \override StaffSymbol.line-count = 1 } { c'4 c c c | c1 | c2 c | c1 }
>> }
% Measures of their own in each staff: where only some have a bar line,
% the stub of a staff without one (allow-span-bar #f, Bar_engraver's)
% keeps the span bar out of it.
\score { \new StaffGroup \with { \remove Timing_translator } <<
  \new Staff \with { \consists Timing_translator } \relative { \time 3/4 c''4 d e f2. g4 f e d2. c2. \bar "|." }
  \new Staff \with { \consists Timing_translator } \relative { \time 5/4 c'4 d e f g a2. g2 f1 e4 \bar "|." }
  \new Staff \with { \consists Timing_translator } \relative { \clef bass \time 3/4 c4 d e f2. g4 f e d2. c2. \bar "|." }
>> }
