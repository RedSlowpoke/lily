%% The columns' skylines' capture (MS3 step 30): the skylines of
%% fixtures/columns.ly's paper columns, written by
%% tools/bench/probe-columns.scm in LilyPond 2.26.0:
%%
%%   lilypond -s tools/compare/columns-calls.ly
%%
%% The file includes the fixture unchanged and adds a recorder to its Score
%% that notes each column the Score's currentCommandColumn and
%% currentMusicalColumn name at the end of each timestep and, in its
%% finalize, writes the probe's lines of their 'horizontal-skylines
%% (columns-run) into columns.txt in the current directory. The probe is
%% found beside this file, by the path LilyPond was given.
%% tools/bench/test/columns.test.mjs compiles the same in the engine, and
%% compares. No page is written.
%%
%% The page's lookup of the System's vertical alignment, which the fixture
%% removes, is a programming error that LilyPond expects here; the engine
%% lays out no page yet.
\version "2.26.0"
#(ly:set-option 'print-pages #f)
#(ly:expect-warning "didn't find a vertical alignment in this system")
#(let ((module (current-module))
       (here (dirname (car (ly:input-file-line-char-column (*location*))))))
   (primitive-load (string-append here "/../bench/probe-columns.scm"))
   (set-current-module module))

#(define (columns-recorder context)
   (let ((columns '()))
     (make-engraver
      ((stop-translation-timestep engraver)
       (for-each (lambda (property)
                   (let ((column (ly:context-property context property)))
                     (if (and (ly:grob? column) (not (memq column columns)))
                         (set! columns (cons column columns)))))
                 '(currentCommandColumn currentMusicalColumn)))
      ((finalize engraver)
       (with-output-to-file "columns.txt"
         (lambda () ((@@ (lily) columns-run) (reverse columns))))))))

\layout {
  \context { \Score \consists #columns-recorder }
}

\include "fixtures/columns.ly"
