;;; b6-strings.scm, prints 1800000
;; strings and symbols: what markup, property names and format-like code do
(define (pad n) (let ((s (number->string n))) (if (< (string-length s) 5) (string-append (make-string (- 5 (string-length s)) #\0) s) s)))
(define (run n)
  (let loop ((i 0) (total 0))
    (if (= i n) total
        (let* ((s (string-append "note-" (pad i) "-" (symbol->string 'Staff)))
               (sym (string->symbol s))
               (back (symbol->string sym))
               (up (list->string (map char-upcase (string->list (substring back 0 4))))))
          (loop (+ i 1) (+ total (string-length back) (if (string=? up "NOTE") 1 0) (if (eq? sym (string->symbol s)) 1 0)))))))
(display (run 100000)) (newline)
