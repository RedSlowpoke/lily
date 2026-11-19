\version "2.26.0"
#(define (loc m) (let ((o (ly:music-property m 'origin))) (if (ly:input-location? o) (cdr (ly:input-both-locations o)) '-)))
#(define (walk m outer)
   (let ((chain (cons (list (ly:music-property m 'name) (loc m)) outer)))
     (when (memq 'event (ly:music-property m 'types))
       (ly:music-set-property! m 'lily-chain (reverse outer)))
     (let ((e (ly:music-property m 'element))) (when (ly:music? e) (walk e chain)))
     (for-each (lambda (c) (walk c chain)) (ly:music-property m 'elements))))
#(set! toplevel-music-functions
   (append toplevel-music-functions (list (lambda (m) (walk m '()) m))))
#(define (Link_engraver context)
   (make-engraver (acknowledgers ((note-head-interface engraver grob source)
     (let* ((ev (ly:grob-property grob 'cause)))
       (ly:message "at ~a ~a chain=~a" (ly:moment-main (ly:context-current-moment context))
                   (ly:event-property ev 'pitch)
                   (filter (lambda (x) (not (eq? (cadr x) '-))) (ly:event-property ev 'lily-chain '()))))))))
\layout { \context { \Voice \consists #Link_engraver } }
plain = { g'4 a' }
mel = \relative { c''4 \plain }
\score { \new Staff { \mel \repeat unfold 2 \plain } }
