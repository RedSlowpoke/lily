\version "2.26.0"
% Titles (MS3 step 63): the book's title, subtitle and composer over its
% first score, each score's piece and opus over it, top-level markups before
% and between the scores, a tagline of the book's own on its last page, and
% a \bookpart with a header of its own, whose pages are numbered on from the
% first part's; spaced as page elements by the page breaker and the page
% layout (top-markup-spacing, markup-system-spacing, score-markup-spacing,
% markup-markup-spacing).
\header {
  title = "Titles"
  subtitle = "A fixture of page elements"
  composer = "A. Composer"
  tagline = "The book's own tagline"
}
\markup { A top-level markup before the first score }
\score {
  \header { piece = "Piece one" }
  \relative { c''4 d e f | g1 \bar "|." }
}
\markup { A markup between the scores }
\markup \italic { and a second one under it }
\score {
  \header { piece = "Piece two" opus = "Op. 2" }
  \relative { e'4 f g a | b1 \bar "|." }
}
\bookpart {
  \header { subtitle = "The second part" }
  \score { \relative { c'2 e | g1 \bar "|." } }
}
