\version "2.26.0"
% The prints of MS3 step 52 (layout.test.mjs): each TextSpanner's,
% Glissando's, DynamicTextSpanner's and VoiceFollower's stencil written to
% the error port as `PRINT type expr` once ly:line-spanner::print has
% made it, in LilyPond 2.26.0 and in the engine. Text spanners dashed,
% solid, dotted and zigzag, with texts at both ends, arrows, padding,
% stencil-align-dir-y, stencil-offset and a font size, above and below;
% glissandi to accidentals and from dots, between chords, with
% normalized-endpoints and extra-dy, across staves; crescendi and
% diminuendi as text; voice followers across staves. And (OPEN A23)
% ly:line-interface::line on a note head in each style, on staves of two
% staff spaces and thicknesses, as `PRINT LINE style expr`. Each score
% one line, as the engine breaks them before MS3's part 5: no piece of a
% broken spanner.
#(define (printed grob)
   (let ((s (ly:line-spanner::print grob)))
     (format (current-error-port) "PRINT ~a ~s\n" (grob::name grob)
             (if (ly:stencil? s) (ly:stencil-expr s) s))
     s))
#(define (line-probe grob)
   (let ((style (ly:grob-property grob 'style)))
     (for-each
      (lambda (case)
        (ly:grob-set-property! grob 'style (car case))
        (ly:grob-set-property! grob 'thickness (cadr case))
        (format (current-error-port) "PRINT LINE ~a ~s\n" (car case)
                (ly:stencil-expr (ly:line-interface::line grob 0 0 3.5 1.25))))
      '((line 1) (dashed-line 2) (dotted-line 1.5) (zigzag 1) (none 1) (other 0.5)))
     (ly:grob-set-property! grob 'style style)
     (ly:grob-set-property! grob 'thickness '())))
\layout {
  \context {
    \Voice
    \override TextSpanner.stencil = #printed
    \override Glissando.stencil = #printed
    \override DynamicTextSpanner.stencil = #printed
    \override VoiceFollower.stencil = #printed
  }
  \context {
    \Dynamics
    \override TextSpanner.stencil = #printed
  }
}
\score { \relative {
  c''4\startTextSpan d e f\stopTextSpan |
  \override TextSpanner.bound-details.left.text = "rit."
  \override TextSpanner.bound-details.right.text = "a tempo"
  \override TextSpanner.style = #'line
  g4\startTextSpan f e d\stopTextSpan |
  \override TextSpanner.style = #'dotted-line
  \override TextSpanner.bound-details.right.text = ##f
  \override TextSpanner.bound-details.right.arrow = ##t
  \override TextSpanner.bound-details.left.arrow = ##t
  \override TextSpanner.arrow-length = #2
  \override TextSpanner.arrow-width = #0.8
  c4\startTextSpan d e f\stopTextSpan |
  \revert TextSpanner.bound-details.left.arrow
  \override TextSpanner.style = #'zigzag
  \override TextSpanner.bound-details.left.padding = #1
  \override TextSpanner.bound-details.right.padding = #2
  \override TextSpanner.bound-details.left.stencil-align-dir-y = #CENTER
  \override TextSpanner.bound-details.left.stencil-offset = #'(0.5 . 1)
  \override TextSpanner.font-size = #3
  \textSpannerDown g4\startTextSpan f e d\stopTextSpan \bar "|."
} }
\score { \relative {
  c''4\glissando fis \glissando e4. \glissando g8 |
  <c, e>2\glissando <g' bes> |
  \override Glissando.normalized-endpoints = #'(0.2 . 0.7)
  c,4\glissando c'
  \revert Glissando.normalized-endpoints
  \override Glissando.extra-dy = #1.5
  \override Glissando.style = #'dashed-line
  \override Glissando.thickness = #2
  d4\glissando a |
  \override Glissando.bound-details.right.arrow = ##t
  \override Glissando.style = #'line
  c2\glissando g' \bar "|."
} }
\score { \relative {
  c''4\cresc d e f\! | g4\dim f e d\! |
  \override DynamicTextSpanner.style = #'line
  \set crescendoText = \markup { \italic "più forte" }
  c4\cresc d e f\! \bar "|."
} }
\score {
  \new PianoStaff <<
    \new Staff = "up" \relative {
      \showStaffSwitch c''4 d \change Staff = "down" g,, a |
      \change Staff = "up" c''2\glissando \change Staff = "down" c,,, |
      \change Staff = "up" e''1 \bar "|."
    }
    \new Staff = "down" \relative { \clef bass c4 d s2 | s1 | s1 }
  >>
}
\score {
  <<
    \new Staff \relative { c''1 | d1 | e1 \bar "|." }
    \new Dynamics { s1\startTextSpan s1 s1\stopTextSpan }
  >>
}
\score {
  <<
    \new Staff { \once \override NoteHead.after-line-breaking = #line-probe c'1 }
    \new Staff \with {
      \override StaffSymbol.staff-space = #1.5
      \override StaffSymbol.thickness = #2
    } { \once \override NoteHead.after-line-breaking = #line-probe c'1 }
  >>
}
