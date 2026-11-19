\version "2.26.0"
#(define (Span_engraver context)
  (make-engraver (acknowledgers ((note-head-interface engraver grob source)
    (let ((o (ly:event-property (ly:grob-property grob (quote cause)) (quote origin))))
      (ly:message "both=~a fl=~a" (ly:input-both-locations o) (ly:input-file-line-char-column o)))))))
\layout { \context { \Voice \consists #Span_engraver } }
{
	%é
	\markup "ж" c''''' 4-. %x
	fisis'
2 }
