\version "2.26.0"
% A \layout translation's error (MS3 step 35, D132): the second score's
% Scheme engraver throws in process-music, after a warning of its own. As
% init.ly runs its book handler with protected-scheme-parsing off, LilyPond
% reports Guile's error at init.ly:66:2, then the error, and exits with 1
% there: nothing is translated after it and the book has no page.

#(ly:warning "before the translations")

{ c'4 }

\score {
  { c'4 }
  \layout {
    \context {
      \Voice
      \consists
      #`((process-music
          . ,(lambda (engraver)
               (ly:warning "in the translation")
               (car '()))))
    }
  }
}

{ e'4 }
