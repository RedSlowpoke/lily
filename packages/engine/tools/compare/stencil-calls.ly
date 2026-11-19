%% O1's capture (01 §3.7, MS3 steps 2 and 3): the calls of stencils, boxes,
%% transforms and shapes in tools/bench/probe-stencils.scm, run in LilyPond
%% 2.26.0's (lily), each call's record on standard output:
%%
%%   lilypond -s tools/compare/stencil-calls.ly
%%
%% The probe is found beside this file, by the path LilyPond was given.
%% crates/layout/tests/stencils.rs and tools/bench/test/stencils.test.mjs
%% run it here and in the engine, and compare.
%%
%% And the line interface through a grob (step 3): ly:line-interface::line
%% of a note head on two staves, a plain one and one of 1.5 staff spaces and
%% lines twice as thick, with each of probe-line-cases' style properties set
%% (style, thickness, dash-fraction, dash-period, zigzag-width and
%% zigzag-length; () unsets one) from its point to its point, into
%% stencil-lines.txt in the current directory, a line a case: the call
%% crates/layout/tests/stencils.rs makes of layout's line with the staff
%% symbol's line thickness and staff space and the case, a tab, and the
%% stencil as the probe shows it. No page is written.
\version "2.26.0"
#(ly:set-option 'print-pages #f)
#(let ((module (current-module))
       (here (dirname (car (ly:input-file-line-char-column (*location*))))))
   (primitive-load (string-append here "/../bench/probe-stencils.scm"))
   (set-current-module module))

#(define probe-line-cases
   '((() () () () () () (0 . 0) (3 . 0))
     (line 2 () () () () (0 . 0) (3 . 4))
     (foo () () () () () (1 . 1) (-2 . 3))
     (dashed-line () () () () () (0 . 0) (10 . 0))
     (dashed-line 1.5 0.3 2 () () (1 . -1) (7.5 . 2))
     (dashed-line () 0.6 0.5 () () (0 . 0) (0.2 . 0))
     (dashed-line () -1 1 () () (0 . 0) (4 . 3))
     (dashed-line () 2 1 () () (0 . 0) (4 . 3))
     (dashed-line () 0.4 -1 () () (0 . 0) (4 . 0))
     (dashed-line () 0.4 0 () () (0 . 0) (4 . 0))
     (dashed-line () 1/3 1/2 () () (0 . 0) (5 . 5))
     (dashed-line 0.5 () () () () (1 . 1) (1 . 1))
     (dotted-line () () () () () (0 . 0) (5 . 0))
     (dotted-line 3 0.5 0.75 () () (2 . 2) (-3 . -1))
     (zigzag () () () () () (0 . 0) (6 . 1))
     (zigzag 2 () () 1.5 2 (0.5 . 0) (8 . -3))
     (zigzag () () () () 0.3 (0 . 0) (4 . 0))
     (zigzag () () () 0.7 1.2 (0 . 0) (0 . 5))
     (zigzag () () () () () (0 . 0) (0 . 0))
     (none () () () () () (0 . 0) (1 . 1))))
#(call-with-output-file "stencil-lines.txt" (lambda (port) #t))
#(define (probe-lines grob)
   (let ((port (open-file "stencil-lines.txt" "a")))
     (for-each
      (lambda (case)
        (let ((from (list-ref case 6)) (to (list-ref case 7)))
          (for-each (lambda (name value) (ly:grob-set-property! grob name value))
                    '(style thickness dash-fraction dash-period zigzag-width zigzag-length)
                    (list-head case 6))
          (write (list 'probe-line
                       (list 'quote (cons* (ly:staff-symbol-line-thickness grob)
                                           (ly:staff-symbol-staff-space grob)
                                           case)))
                 port)
          (display "\t" port)
          (write ((@@ (lily) probe-show)
                  (ly:line-interface::line grob (car from) (cdr from) (car to) (cdr to)))
                 port)
          (newline port)))
      probe-line-cases)
     (close-port port)))
<<
  \new Staff { \override NoteHead.after-line-breaking = #probe-lines c'1 }
  \new Staff \with {
    \override StaffSymbol.staff-space = #1.5
    \override StaffSymbol.thickness = #2
  } { \override NoteHead.after-line-breaking = #probe-lines c'1 }
>>
