\version "2.26.0"
% A Scheme engraver's finalize error (MS3 step 35, D132): it throws once
% the translation is done, and LilyPond reports it at init.ly:66:2 as any
% error of a translation, and exits with 1 there.

\layout {
  \context {
    \Voice
    \consists
    #`((finalize . ,(lambda (engraver) (error "the engraver's finalize"))))
  }
}

{ c'4 }
