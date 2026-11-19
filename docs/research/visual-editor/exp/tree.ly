\version "2.26.0"
#(define (tree-walk m path)
   (let ((name (ly:music-property m 'name))
         (o (ly:music-property m 'origin)))
     (when (memq name '(NoteEvent RelativeOctaveMusic TransposedMusic UnfoldedRepeatedMusic VoltaRepeatedMusic EventChord))
       (ly:message "~a ~a ~a ~a" (reverse path) name
                   (if (ly:input-location? o) (cdr (ly:input-both-locations o)) "-")
                   (let ((p (ly:music-property m 'pitch))) (if (ly:pitch? p) p ""))))
     (let ((e (ly:music-property m 'element)))
       (when (ly:music? e) (tree-walk e (cons name path))))
     (for-each (lambda (c) (tree-walk c (cons name path))) (ly:music-property m 'elements))))
#(set! toplevel-music-functions
   (append toplevel-music-functions
           (list (lambda (m) (ly:message "--- toplevel music") (tree-walk m '()) m))))
mel = \relative { c''4 d \transpose c d { e } }
\score { \new Staff { \mel \repeat volta 2 \mel } }
{ \mel }
