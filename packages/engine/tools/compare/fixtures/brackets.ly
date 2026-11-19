\version "2.26.0"
% OttavaBracket and HorizontalBracket (MS3 step 47, for step 49): ottava
% brackets up and down (ottava-bracket.cc over bracket.cc), and analysis
% brackets of a voice (Horizontal_bracket_engraver, horizontal-bracket.cc),
% nested and with their texts, each score one line.
\score { \relative {
  c''4 d \ottava #1 e f | g a \ottava #0 b c |
  \ottava #-1 c,,4 d e f \ottava #0 | \ottava #2 c''''4 d \ottava #0 e2 \bar "|."
} }
\score {
  \new Voice \with { \consists "Horizontal_bracket_engraver" } \relative {
    c''4\startGroup d\stopGroup e\startGroup\startGroup f\stopGroup |
    g4\stopGroup
    \override HorizontalBracketText.text = "a"
    a\startGroup b c\stopGroup \bar "|."
  }
}
