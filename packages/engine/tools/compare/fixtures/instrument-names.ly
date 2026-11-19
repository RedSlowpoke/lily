\version "2.26.0"
% InstrumentName (MS3 step 43): instrument and vocal names and their short
% names (Instrument_name_engraver), printed and placed at the system's
% start against its delimiters by output-lib.scm's system-start-text::
% callbacks, each score one line, which the engine lays out whole (a short
% name prints only on a later line, which the engine's one line has not:
% here they are made and killed as in LilyPond's). No PianoStaff: a
% brace's font is in no preloaded pack.
% Names and short names; a name set mid-line, made and killed by its
% print (its left bound no line's start).
\score { <<
  \new Staff \with { instrumentName = "Violin" shortInstrumentName = "Vl." }
    \relative { c''4 d e f | g a b c | \set Staff.instrumentName = "Viola" c1 \bar "|." }
  \new Staff \with { instrumentName = "Violoncello" shortInstrumentName = "Vc." }
    \relative { \clef bass c4 b a g | f e d c | c1 }
>> }
% Groups in a group, each named: a markup name, vocalName where neither
% instrument name is set, a short name alone (killed at the start, its
% long name no markup), a name aligned on its right.
\score { \new StaffGroup \with { instrumentName = "Strings" } <<
  \new Staff \with { instrumentName = \markup \column { "Viola" "da gamba" } }
    \relative { \clef alto c'4 d e f | g1 }
  \new ChoirStaff \with { instrumentName = "Choir" } <<
    \new Staff \with { vocalName = "Soprano" shortVocalName = "S." } \relative { c''4 d e f | g1 }
    \new Staff \with { shortInstrumentName = "A." } \relative { c'4 d e f | g1 }
  >>
  \new Staff \with { instrumentName = "Bass" \override InstrumentName.self-alignment-X = #RIGHT }
    \relative { \clef bass c4 d e f | g1 }
>> }
% No indent: a name wider than the indent, aligned on its left, padded,
% and one aligned on its top.
\score { <<
  \new Staff \with {
    instrumentName = "Contrabassoon"
    \override InstrumentName.self-alignment-X = #LEFT
    \override InstrumentName.padding = #1.5
  } \relative { \clef bass c4 d e f | g1 }
  \new Staff \with {
    instrumentName = \markup \column { "Horn" "in F" }
    \override InstrumentName.self-alignment-Y = #UP
  } \relative { c''4 d e f | g1 }
>>
  \layout { indent = 0 }
}
