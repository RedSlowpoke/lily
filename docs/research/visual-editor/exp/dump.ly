\version "2.26.0"
%% Dump, for every grob, its name, the context that made it, the moment, and its cause.
#(define (loc->string loc)
   (if (ly:input-location? loc)
       (format #f "~a" (cdr (ly:input-both-locations loc)))
       "-"))
#(define (cause-info grob)
   (let ((c (ly:grob-property grob 'cause)))
     (cond ((ly:stream-event? c)
            (format #f "event ~a span=~a"
                    (ly:event-property c 'class)
                    (loc->string (ly:event-property c 'origin))))
           ((ly:grob? c) (format #f "grob ~a" (grob::name c)))
           (else (format #f "~a" c)))))
#(define (Dump_engraver context)
   (make-engraver
    (acknowledgers
     ((grob-interface engraver grob source-engraver)
      (let ((ctx (ly:translator-context source-engraver)))
        (ly:message "~a ctx=~a/~a at=~a bar=~a pos=~a | ~a"
                    (grob::name grob)
                    (ly:context-name ctx) (ly:context-id ctx)
                    (ly:context-current-moment context)
                    (ly:context-property ctx 'currentBarNumber)
                    (ly:context-property ctx 'measurePosition)
                    (cause-info grob)))))))
\layout { \context { \Score \consists #Dump_engraver } }
mel = \relative c'' { c4-. d( <e g>8) f\p r4 | \repeat unfold 2 { g16 a } b2. }
\score { \new Staff = "up" \new Voice = "v1" { \mel } }
