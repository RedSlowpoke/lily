\version "2.26.0"
#(define meta-counter 0)
#(define (meta-attrs grob)
   ;; Evaluated when the backend asks, once per grob and per broken piece.
   (set! meta-counter (1+ meta-counter))
   (let* ((orig (ly:grob-original grob))
          (pieces (if (ly:spanner? grob) (length (ly:spanner-broken-into orig)) 0))
          (col (lambda (dir) (ly:grob-property (ly:item-get-column (ly:spanner-bound grob dir)) 'when #f))))
     `((id . ,(format #f "g~a" meta-counter))
       (data-grob . ,(grob::name grob))
       (data-pieces . ,pieces)
       ,@(if (ly:spanner? grob)
             (let ((l (ly:spanner-bound grob LEFT)) (r (ly:spanner-bound grob RIGHT)))
               `((data-from . ,(if (ly:item? l) (ly:grob-property (ly:item-get-column l) 'when) "?"))
                 (data-to . ,(if (ly:item? r) (ly:grob-property (ly:item-get-column r) 'when) "?"))))
             '()))))
\layout { \context { \Voice
  \override Slur.output-attributes = #meta-attrs
  \override Tie.output-attributes = #meta-attrs
  \override NoteHead.output-attributes = #meta-attrs } }
{ c'1( d' \break e' f') g'~ \break g' \grace { a'16 } b'4 }
