%% The spacer's capture (MS3 step 29): the calls of springs and spring-rod
%% problems in tools/bench/probe-spacer.scm, run in LilyPond 2.26.0's
%% (lily), each call's line on standard output:
%%
%%   lilypond -s tools/compare/spacer-calls.ly
%%
%% The probe is found beside this file, by the path LilyPond was given.
%% tools/bench/test/spacer.test.mjs runs it here and in the engine, and
%% compares. No page is written.
\version "2.26.0"
#(ly:set-option 'print-pages #f)
#(let ((module (current-module))
       (here (dirname (car (ly:input-file-line-char-column (*location*))))))
   (primitive-load (string-append here "/../bench/probe-spacer.scm"))
   (set-current-module module))
