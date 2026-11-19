\version "2.26.0"
% Font sizes from the context (MS3 step 48, OPEN A34): Font_size_engraver
% puts fontSize into its context's grobs' font-size. \magnifyStaff in a
% \with and in the music, fontSize set, \small and \teeny, and a staff
% magnified above another (step 42's probe, whose Y offsets were off
% LilyPond's). Its sizes need emmentaler-13, -14 and -16, no preloaded
% pack (W5's packs on demand).
\score {
  <<
    \new Staff \with { \magnifyStaff #2/3 } { c'4 d' e' f' | g'1 }
    \new Staff { \magnifyStaff #3/4 c'4 d' \small e' f' | \normalsize g'1 }
    \new Staff \with { fontSize = #-2 } { c'4 \teeny d' e' \set fontSize = #-1 f' | g'1 }
  >>
}
\score {
  \new StaffGroup \with { \remove Span_bar_engraver } <<
    \new Staff = "low" \relative { \clef bass c4 d e f | g1 | a2 g | f1 \bar "|." }
    \new Staff \with { alignAboveContext = "low" \magnifyStaff #2/3 } \relative { c'4 d e f | g1 | a2 g | f1 }
    \new Staff \with { alignAboveContext = "low" \override StaffSymbol.line-count = 1 } { c'4 c c c | c1 | c2 c | c1 }
  >>
}
