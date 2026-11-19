\version "2.26.0"
% An error of a \book block's pages (MS3 step 64, D220): the first book's
% header markup throws when its page is drawn. The parser calls a \book
% block's handler unprotected, so LilyPond prints Guile's report of the
% error (whose first line names a frame in Guile's own files, which the
% engine leaves out) and exits with 1 at once: the second book is not
% translated, so its warning is not printed and it has no page.

#(define-markup-command (broken layout props) () (car '()))

\book {
  \paper { oddHeaderMarkup = \markup \broken }
  { c'4 }
}

\book {
  \score {
    { d'4 }
    \layout {
      \context {
        \Voice
        \consists
        #`((process-music
            . ,(lambda (engraver) (ly:warning "in the second book"))))
      }
    }
  }
}
