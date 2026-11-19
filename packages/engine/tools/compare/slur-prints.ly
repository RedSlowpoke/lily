\version "2.26.0"
% The prints of MS4 step 7 (layout.test.mjs): each Slur's and PhrasingSlur's
% stencil written to the error port as `PRINT type key control-points
% expr` once ly:slur::print has made it, for the scores of
% fixtures/slurs.ly. The key is the slur's cause's line and column, its
% direction and its piece of a broken slur (-1 for an unbroken one). In
% LilyPond the control points are its own; in the engine, before the
% slur scoring (MS4 step 8), they are LilyPond's, read by key from
% `slur-control-points`, an alist of key and control points that a file
% including this one defines first.
#(define (slur-key grob)
   (let* ((ev (event-cause grob))
          (loc (ly:input-file-line-char-column (ly:event-property ev 'origin)))
          (orig (ly:grob-original grob))
          (pieces (if (and orig (not (eq? orig grob)))
                      (ly:spanner-broken-into orig)
                      '()))
          (piece (let loop ((l pieces) (i 0))
                   (cond ((null? l) -1)
                         ((eq? (car l) grob) i)
                         (else (loop (cdr l) (1+ i)))))))
     (list (cadr loc) (caddr loc) (ly:grob-property grob 'direction) piece)))
#(define slur-table
   (if (defined? 'slur-control-points) slur-control-points '()))
#(define (control-points grob)
   (let ((given (assoc (slur-key grob) slur-table)))
     (if given (cdr given) (ly:slur::calc-control-points grob))))
#(define (printed grob)
   (let ((s (ly:slur::print grob)))
     (format (current-error-port) "PRINT ~a ~s ~s ~s\n" (grob::name grob)
             (slur-key grob) (ly:grob-property grob 'control-points)
             (if (ly:stencil? s) (ly:stencil-expr s) s))
     s))
\layout {
  \context {
    \Voice
    \override Slur.control-points = #control-points
    \override Slur.stencil = #printed
    \override PhrasingSlur.control-points = #control-points
    \override PhrasingSlur.stencil = #printed
  }
}
\include "fixtures/slurs.ly"
