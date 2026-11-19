\version "2.26.0"
% MultiMeasureRest (MS3 step 42): multi-measure rests engraved from their
% events (Multi_measure_rest_engraver) and printed and spaced as
% multi-measure-rest.cc does them, with their numbers, texts and scripts,
% each score one line, which the engine lays out whole. Each count is one
% whose digits Emmentaler does not kern, as it was before its packs held
% GPOS's kern (W6, MS3 step 51: since then 10, 14, 16, 21, 30, 99 and 100
% are LilyPond's widths too, its notes' probe).
% Compressed rests of 2 to 25 measures, and of one (no number).
\score { \relative {
  \compressEmptyMeasures
  R1*2 | R1*3 | R1*5 | R1*12 | R1*25 | R1 | c''1 \bar "|."
} }
% In 3/4 and 2/2.
\score { \relative {
  \compressEmptyMeasures
  \time 3/4 R2.*4 | R2. |
  \time 2/2 R1*7 | c''1 \bar "|."
} }
% restNumberThreshold 0 and 3; a number's text set by hand.
\score { \relative {
  \compressEmptyMeasures
  \set Score.restNumberThreshold = 0
  R1 | R1*2 |
  \set Score.restNumberThreshold = 3
  R1*3 | R1*4 |
  \unset Score.restNumberThreshold
  \once \override MultiMeasureRestNumber.text = "9" R1*2 |
  c''1 \bar "|."
} }
% Texts and scripts: a text above the number; a script and a text above
% it, stacked; a script alone; two scripts stacked; a script below; a rest
% of three measures not compressed, one a measure. (No text touches
% another text, or lies below the staff or beside one, where the engine's
% text skylines, boxes without W1's outlines, are not LilyPond's.)
\score { \relative {
  \compressEmptyMeasures
  R1*3^"tacet" | R1*2^\fermata^"tacet" | R1\fermata |
  R1*2^\fermata^\fermata | R1*2_\fermata |
  \expandEmptyMeasures
  R1*3 | c''1 \bar "|."
} }
