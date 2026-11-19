\version "2.26.0"
#(define (Props_engraver context)
   (make-engraver
    ((initialize engraver)
     (ly:add-listener
      (lambda (ev)
        (let ((o (ly:event-property ev 'origin)))
          (ly:message "SetProperty ~a at ~a origin=~a" (ly:event-property ev 'symbol)
                      (ly:moment-main (ly:context-current-moment context))
                      (if (ly:input-location? o) (cdr (ly:input-both-locations o)) '-))))
      (ly:context-events-below context) 'SetProperty))
    (acknowledgers ((grob-interface engraver grob source)
      (when (memq (grob::name grob) '(Clef KeySignature TimeSignature KeyCancellation))
        (let ((c (ly:grob-property grob 'cause)))
          (ly:message "grob ~a cause=~a" (grob::name grob)
            (if (ly:stream-event? c) (cdr (ly:input-both-locations (ly:event-property c 'origin))) c))))))))
\layout { \context { \Score \consists #Props_engraver } }
{ \clef bass \key d \major \time 3/4 c4 d e | \time 2/4 \clef treble f g }
