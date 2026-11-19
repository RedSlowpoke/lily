\version "2.26.0"
% TextSpanner, Glissando, DynamicTextSpanner and VoiceFollower (MS3 step
% 47, for step 52): line spanners (line-spanner.cc) with their bound
% texts and arrows, each score one line.
% A text spanner, dashed and solid with an arrow; glissandi straight, a
% zigzag one and one between chords; a crescendo and a diminuendo as text.
\score { \relative {
  c''4\startTextSpan d e f\stopTextSpan |
  \override TextSpanner.bound-details.left.text = "rit."
  \override TextSpanner.style = #'line
  \override TextSpanner.bound-details.right.arrow = ##t
  g4\startTextSpan f e d\stopTextSpan |
  c4\glissando g' \override Glissando.style = #'zigzag a\glissando c, |
  <c e>2\glissando <g' b> |
  c,4\cresc d e f\! | g4\dim f e d\! \bar "|."
} }
% A voice that changes staff, its follower line shown; a text spanner
% below the staff.
\score {
  \new StaffGroup <<
    \new Staff = "up" \relative {
      \showStaffSwitch c''4 d \change Staff = "down" g,, a | \change Staff = "up" c''1 \bar "|."
    }
    \new Staff = "down" \relative { \clef bass c4 d s2 | \textSpannerDown g\startTextSpan a b c\stopTextSpan }
  >>
}
