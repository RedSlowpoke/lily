\version "2.26.0"
% An error of the pages (MS3 step 64, D220): the header's markup throws
% when the page is drawn, after the score's translation has warned. As
% init.ly runs its book handler with protected-scheme-parsing off, LilyPond
% reports Guile's error at init.ly:66:2, then the error, and exits with 1
% there: the book has no page and the run no MIDI file.

#(define-markup-command (broken layout props) () (car '()))

\paper { oddHeaderMarkup = \markup \broken }

\score {
  { c'4 }
  \layout {
    \context {
      \Voice
      \consists
      #`((process-music
          . ,(lambda (engraver) (ly:warning "in the translation"))))
    }
  }
  \midi { }
}
