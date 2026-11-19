%% Reference capture only (chapter 00 §5.1, Appendix A): every score of every
%% book gets a \midi performance, so timing.ly writes the score's events even
%% when it asks for no MIDI. A score without output definitions keeps its
%% default \layout. c5dump.ly writes C5's beam, slur and tie shapes after page
%% breaking, stencils.ly the pages' stencils and the markups' (MS3 step 6),
%% grobs.ly the grobs each translation announces (MS3 step 12), and layout.ly
%% the live grobs of each system after page breaking (MS3 step 25), with
%% grobs.ly's fields. capture.mjs puts @lily/common's runtime/ (timing.ly) and
%% this directory (c5dump.ly, stencils.ly, grobs.ly, layout.ly) on the include
%% path.
\include "timing.ly"
\include "c5dump.ly"
\include "stencils.ly"
\include "grobs.ly"
\include "layout.ly"
%% Text in LilyPond's own fonts (D106): with --svg, paper-defaults-init.ly
%% gives fontconfig's serif, sans and monospace, which are the machine's; every
%% other backend gives these, C059 among them.
\paper {
  property-defaults.fonts.serif = "LilyPond Serif"
  property-defaults.fonts.sans = "LilyPond Sans Serif"
  property-defaults.fonts.typewriter = "LilyPond Monospace"
}
#(define (lily-capture-kind def) (ly:output-def-lookup def 'output-def-kind #f))
#(define (lily-capture-score! score)
   (let ((defs (ly:score-output-defs score)))
     (unless (any (lambda (def) (eq? (lily-capture-kind def) 'midi)) defs)
       (when (null? defs) (ly:score-add-output-def! score (ly:output-def-clone $defaultlayout)))
       (ly:score-add-output-def! score (ly:output-def-clone $defaultmidi)))))
#(define (lily-capture-book! book)
   (for-each (lambda (score) (when (ly:score? score) (lily-capture-score! score))) (ly:book-scores book))
   (for-each lily-capture-book! (ly:book-book-parts book)))
#(define lily-capture-book-handler toplevel-book-handler)
#(define (toplevel-book-handler book) (lily-capture-book! book) (lily-capture-book-handler book))
%% Layout is compared through the point-and-click links, so a score may not turn them off.
#(define lily-capture-set-option ly:set-option)
#(define (ly:set-option key . rest)
   (unless (eq? key 'point-and-click) (apply lily-capture-set-option key rest)))
%% A link names a file given relatively (T16's included string) below
%% ly-getcwd, the directory -o changes into (main.cc), which differs from one
%% capture to another. capture.mjs sets CAPTURE_CWD to a fixed directory for
%% each score, the one a capture into ref/ writes into, so that the links are
%% the same wherever the capture goes (MS3 step 12a). output-svg.scm's and
%% output-ps.scm's grob-cause and stencils.ly's links are its only callers.
#(let ((cwd (getenv "CAPTURE_CWD")))
   (when cwd (module-set! (resolve-module '(lily)) 'ly-getcwd (lambda () cwd))))
