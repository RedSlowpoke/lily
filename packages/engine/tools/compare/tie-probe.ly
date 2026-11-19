%% MS4 step 9's probe of the ties' formatting (tie-formatting-problem.cc):
%% included before a score (layout.test.mjs writes a file that includes
%% this, then the score), it records each tie column's ties as their
%% positioning leaves them in LilyPond 2.26.0 and in the engine alike, one
%% line a tie, into tie-probe.txt in the current directory (the engine's
%% outputs), the file written again with every line, as the engine has no
%% files to append to:
%%
%%   link  piece  index  direction  control-points
%%
%% link            the origin of the event the tie column's cause leads to,
%%                 file:line:char:column as grobs.ly writes it
%% piece           the column's piece of a broken column, -1 for an
%%                 unbroken one
%% index           the tie's index in the column's ties
%% direction       the tie's direction once positioned
%% control-points  its four points, x and y, as positioned
%%
%% The positioning is triggered in each TieColumn's after-line-breaking
%% (it has none of its own), its before-line-breaking LilyPond's; the
%% ties draw nothing (stencil #f), so that nothing but the probe reads
%% them. In the engine before MS4 step 10 the tie column's callbacks are
%% the music crate's placeholders, so the probe calls the engine's
%% formatting through %host-call (D127), which LilyPond has not.
%% Numbers are written whole, as `number->string' writes them, so that a
%% last bit shows.
#(begin
  (define tie-probe-lines '())
  (define (tie-probe-line . xs)
    (set! tie-probe-lines (cons (string-join xs "\t") tie-probe-lines))
    (with-output-to-file "tie-probe.txt"
      (lambda ()
        (for-each (lambda (l) (display l) (newline)) (reverse tie-probe-lines)))))
  (define tie-probe-engine (defined? '%host-call))
  (define (tie-probe-call name grob)
    (if tie-probe-engine
        ((eval '%host-call (current-module)) 'tie-column name grob)
        ((if (eq? name 'before-line-breaking)
             ly:tie-column::before-line-breaking
             ly:tie-column::calc-positioning-done)
         grob)))
  (define (tie-probe-link grob)
    (let* ((cause (let loop ((cause (ly:grob-property grob 'cause)))
                    (if (ly:grob? cause) (loop (ly:grob-property cause 'cause)) cause)))
           (origin (and (ly:stream-event? cause) (ly:event-property cause 'origin #f))))
      (if (ly:input-location? origin)
          (let ((l (ly:input-file-line-char-column origin)))
            (format #f "~a:~a:~a:~a" (basename (car l)) (cadr l) (caddr l) (cadddr l)))
          "-")))
  (define (tie-probe-piece grob)
    (let* ((orig (ly:grob-original grob))
           (pieces (if (and orig (not (eq? orig grob))) (ly:spanner-broken-into orig) '())))
      (let loop ((l pieces) (i 0))
        (cond ((null? l) -1)
              ((eq? (car l) grob) i)
              (else (loop (cdr l) (1+ i)))))))
  (define (tie-probe-points cps)
    (if (pair? cps)
        (string-join
         (map (lambda (p) (string-append (number->string (car p)) " " (number->string (cdr p)))) cps)
         " ")
        (format #f "~a" cps)))
  (define (tie-probe-column col)
    (tie-probe-call 'calc-positioning-done col)
    (let ((ties (ly:grob-object col 'ties #f)))
      (if (ly:grob-array? ties)
          (let loop ((ts (ly:grob-array->list ties)) (i 0))
            (if (pair? ts)
                (let ((t (car ts)))
                  (tie-probe-line
                   (tie-probe-link col) (number->string (tie-probe-piece col)) (number->string i)
                   (format #f "~a" (ly:grob-property-data t 'direction))
                   (tie-probe-points (ly:grob-property-data t 'control-points)))
                  (loop (cdr ts) (1+ i)))))))))
\layout {
  \context {
    \Score
    \override TieColumn.before-line-breaking = #(lambda (g) (tie-probe-call 'before-line-breaking g))
    \override TieColumn.after-line-breaking = #tie-probe-column
    \override Tie.stencil = ##f
  }
}
