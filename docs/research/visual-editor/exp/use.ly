\version "2.26.0"
plain = { g'4 a' }
mel = \relative { c''4 \plain }
\score { \new Staff { \mel \repeat unfold 2 \plain } }
