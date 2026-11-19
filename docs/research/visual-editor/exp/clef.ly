\version "2.26.0"
#(define (clef-set? m)
   (and (eq? (ly:music-property m 'name) 'ContextSpeccedMusic)
        (let ((e (ly:music-property m 'element)))
          (and (ly:music? e)
               (any (lambda (x) (eq? (ly:music-property x 'symbol) 'clefGlyph))
                    (cons e (ly:music-property e 'elements)))))))
#(define (mark-clefs m)
   (music-map
    (lambda (x)
      (if (clef-set? x)
          (let ((o (ly:music-property x 'origin)))
            (make-sequential-music
             (list x (make-music 'ApplyContext 'origin o
                       'procedure (lambda (ctx)
                                    (ly:message "clef ~a at ~a in ~a/~a" (ly:context-property ctx 'clefGlyph)
                                                (ly:moment-main (ly:context-current-moment ctx))
                                                (ly:context-name ctx) (cdr (ly:input-both-locations o))))))))
          x))
    m))
#(set! toplevel-music-functions (append toplevel-music-functions (list mark-clefs)))
{ \clef bass c4 d e f | \clef treble g a b c' }
