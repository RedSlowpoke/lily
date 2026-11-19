%% O2's capture (01 §3.7, MS3 steps 4 and 5): the calls of LilyPond's font
%% metrics in tools/bench/probe-fonts.scm, on emmentaler-20, emmentaler-13
%% and emmentaler-brace as ly:system-font-load loads them from LilyPond
%% 2.26.0's OTFs, run in its (lily), each call's record on standard output:
%%
%%   lilypond -s tools/compare/font-calls.ly
%%
%% With FONT_PROBE, the probe of that file instead: tools/bench/test/
%% fonts.test.mjs gives probe-select.scm's calls of ly:paper-get-font so,
%% after the cases of the 10 scores. The probe finds the default \paper as
%% (lily)'s probe-paper. The default probe is found beside this file, by the
%% path LilyPond was given. crates/layout/tests/fonts.rs and
%% tools/bench/test/fonts.test.mjs run the probes in the engine on W1's
%% packs of the same fonts, and compare. LilyPond's programming errors (a
%% character asked of an index the cmap has not) go to standard error,
%% which is not compared. No page is written.
\version "2.26.0"
#(ly:set-option 'print-pages #f)
#(let ((module (current-module))
       (here (dirname (car (ly:input-file-line-char-column (*location*))))))
   (module-define! (resolve-module '(lily)) 'probe-paper $defaultpaper)
   (primitive-load (or (getenv "FONT_PROBE") (string-append here "/../bench/probe-fonts.scm")))
   (set-current-module module))
