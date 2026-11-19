\version "2.26.0"
#(define music-uses (make-hash-table))
#(define (loc m) (let ((o (ly:music-property m 'origin))) (if (ly:input-location? o) (cdr (ly:input-both-locations o)) '-)))
#(define (walk m outer)
   ;; Remember, for every event, the chain of enclosing music with a location.
   (let ((chain (if (memq (ly:music-property m 'name) '(SequentialMusic SimultaneousMusic EventChord)) outer
                    (cons (list (ly:music-property m 'name) (loc m)) outer))))
     (when (memq 'event (ly:music-property m 'types)) (hashq-set! music-uses m chain))
     (let ((e (ly:music-property m 'element))) (when (ly:music? e) (walk e chain)))
     (for-each (lambda (c) (walk c chain)) (ly:music-property m 'elements))
     (for-each (lambda (c) (walk c chain)) (ly:music-property m 'articulations))))
#(set! toplevel-music-functions
   (append toplevel-music-functions (list (lambda (m) (walk m '()) m))))
#(define (Link_engraver context)
   (make-engraver (acknowledgers ((note-head-interface engraver grob source)
     (let* ((ev (ly:grob-property grob 'cause))
            (m (ly:event-property ev 'music-cause)))
       (ly:message "at ~a ~a chain=~a" (ly:moment-main (ly:context-current-moment context))
                   (ly:event-property ev 'pitch) (reverse (cdr (or (hashq-ref music-uses m) '(#f #f)))) ))))))
\layout { \context { \Voice \consists #Link_engraver } }
plain = { g'4 a' }
mel = \relative { c''4 \plain }
\score { \new Staff { \mel \repeat unfold 2 \plain } }
