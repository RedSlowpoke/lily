%% Skylines' capture (MS3 step 26): the ly:skyline-* calls on built skylines
%% in tools/bench/probe-skylines.scm, run in LilyPond 2.26.0's (lily), each
%% call's line on standard output:
%%
%%   lilypond -s tools/compare/skyline-calls.ly
%%
%% The probe is found beside this file, by the path LilyPond was given.
%% tools/bench/test/skylines.test.mjs runs it here and crates/skyline's
%% tests/skyline.rs runs its calls on the port, and compares.
%%
%% And the default skylines of fixtures/grobs.ly's grobs: the file includes
%% the fixture unchanged and adds a recorder to its Score that hears every
%% grob as the Score's engravers acknowledge them and, in its finalize,
%% writes the probe's lines of their 'vertical-skylines and
%% 'horizontal-skylines (skyline-grobs-run) into skyline-grobs.txt in the
%% current directory. skylines.test.mjs compiles the same in the engine.
%% No page is written.
\version "2.26.0"
#(ly:set-option 'print-pages #f)
#(let ((module (current-module))
       (here (dirname (car (ly:input-file-line-char-column (*location*))))))
   (primitive-load (string-append here "/../bench/probe-skylines.scm"))
   (set-current-module module))

#(define (skyline-grobs-recorder context)
   (let ((heard '()))
     (make-engraver
      (acknowledgers
       ((grob-interface engraver grob source-engraver)
        (set! heard (cons grob heard))))
      ((finalize engraver)
       (with-output-to-file "skyline-grobs.txt"
         (lambda () ((@@ (lily) skyline-grobs-run) (reverse heard))))))))

\layout {
  \context { \Score \consists #skyline-grobs-recorder }
}

\include "fixtures/grobs.ly"
