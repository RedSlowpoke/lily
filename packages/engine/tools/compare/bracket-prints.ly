\version "2.26.0"
% The prints of MS3 step 49 (layout.test.mjs): each Hairpin's,
% VoltaBracket's, OttavaBracket's and HorizontalBracket's stencil written
% to the error port as `PRINT type expr` once its print has made it, in
% LilyPond 2.26.0 and in the engine. Hairpins growing and shrinking,
% circled, back to back, at rests and dynamics, aligned and shortened;
% volta brackets, numbered (Emmentaler's shaping, W6) and of texts in
% the text font;
% ottava brackets up and down; analysis brackets nested. Each score one
% line, as the engine breaks them before MS3's part 5: no piece of a
% broken spanner.
#(define ((printed print) grob)
   (let ((s (print grob)))
     (format (current-error-port) "PRINT ~a ~s\n" (grob::name grob)
             (if (ly:stencil? s) (ly:stencil-expr s) s))
     s))
\layout {
  \context {
    \Score
    \override VoltaBracket.stencil = #(printed ly:volta-bracket-interface::print)
    \override VoltaBracket.font-encoding = #'latin1
  }
  \context {
    \Voice
    \consists "Horizontal_bracket_engraver"
    \override Hairpin.stencil = #(printed ly:hairpin::print)
    \override HorizontalBracket.stencil = #(printed ly:horizontal-bracket::print)
  }
  \context {
    \Staff
    \override OttavaBracket.stencil = #(printed ly:ottava-bracket::print)
  }
}
\score { \relative { c''4\< d e f\! | g\> a b c\! | c,\p\< d e\f\> f | g1\! } }
\score { \relative {
  \once \override Hairpin.circled-tip = ##t c''4\< d e f\! |
  \once \override Hairpin.circled-tip = ##t g\> a b c\! |
  \once \override Hairpin.circled-tip = ##t c\> d\< e f\!
} }
\score { \relative { c''4\< d r2\! | c4\p\< d e\f r | c4\> d e f\! } }
\score { \relative {
  \once \override Hairpin.endpoint-alignments = #'(0 . 0) c''4\< d e f\! |
  \once \override Hairpin.shorten-pair = #'(1 . -1) g\> a b c\!
} }
\score { \relative {
  \repeat volta 2 { c''4 d e f } \alternative { { g a b c } { c, d e f } } \bar "|."
} }
\score { \relative {
  c''4 d e f | \set Score.repeatCommands = #'((volta "1st")) g a b c |
  \set Score.repeatCommands = #'((volta #f) (volta "2nd and 3rd") end-repeat) c, d e f |
  \set Score.repeatCommands = #'((volta #f)) g1 \bar "|."
} }
\score { \relative {
  c''4 d \ottava #1 e f | g a \ottava #0 b c |
  \ottava #-1 c,,4 d e f \ottava #0 | \ottava #2 c''''4 d \ottava #0 e2 \bar "|."
} }
\score { \relative {
  c''4\startGroup d\stopGroup e\startGroup\startGroup f\stopGroup | g\stopGroup a b c |
  c4\startGroup d e f | g a b c\stopGroup \bar "|."
} }
