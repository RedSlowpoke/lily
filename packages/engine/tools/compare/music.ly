%% The music trees of a score as the book handler gets them, for the parser's
%% steps of MS2 (04 §6, Appendix A's c4.ly in census mode): capture.mjs
%% --parse compiles each score with this file as a settings file and writes
%% the trees to MUSIC_OUT. The book is not handled further: nothing is laid
%% out or performed.
%%
%% One line per node, property and value, indented two spaces a level:
%%
%%   score 1
%%   SequentialMusic @t.ly:2:0-2:18
%%     elements:
%%       NoteEvent @t.ly:2:2-2:4
%%         duration = #<Duration 4 >
%%         pitch = #<Pitch c' >
%%
%% A node is its type's name and its origin, the file's base name and the
%% first and last line and column of its text (`ly:input-both-locations`).
%% Its mutable properties follow, sorted by name, the origin left out: music
%% and lists of music as nodes below the property's name, any other value as
%% `write` writes it, but procedures by their names, context mods by their
%% lists, unpure-pure containers by their parts, origins as above, and the
%% addresses Guile writes taken out. A node met again (the same
%% object) is written once more as `= Name @origin`, without its properties.
#(begin
  (use-modules (ice-9 regex))
  (define music-port (open-output-file (or (getenv "MUSIC_OUT") "music.txt")))
  (define music-scores 0)
  (define (music-origin loc)
    (if (ly:input-location? loc)
        (let ((at (ly:input-both-locations loc)))
          (format #f "@~a:~a:~a-~a:~a" (basename (list-ref at 0))
                  (list-ref at 1) (list-ref at 2) (list-ref at 3) (list-ref at 4)))
        "@?"))
  (define (music-written v)
    (regexp-substitute/global #f " (0x)?[0-9a-f]{8,}" (object->string v) 'pre 'post))
  (define (music-value v)
    (cond ((ly:input-location? v) (music-origin v))
          ((procedure? v)
           (let ((name (procedure-name v)))
             (if name (format #f "#<procedure ~a>" name) "#<procedure>")))
          ((ly:context-mod? v)
           (string-append "#<context-mod " (music-value (ly:get-context-mods v)) ">"))
          ((ly:unpure-pure-container? v)
           (string-append "#<unpure-pure-container "
                          (music-value (ly:unpure-pure-container-unpure-part v)) " "
                          (music-value (ly:unpure-pure-container-pure-part v)) ">"))
          ((ly:music? v) (format #f "#<music ~a>" (ly:music-property v 'name)))
          ((pair? v)
           (let loop ((v v) (parts '()))
             (cond ((pair? v) (loop (cdr v) (cons (music-value (car v)) parts)))
                   ((null? v) (string-append "(" (string-join (reverse parts) " ") ")"))
                   (else (string-append "(" (string-join (reverse parts) " ")
                                        " . " (music-value v) ")")))))
          ((vector? v) (string-append "#" (music-value (vector->list v))))
          ((string? v) (object->string v))
          (else (music-written v))))
  (define (music-line depth . parts)
    (display (make-string (* 2 depth) #\space) music-port)
    (for-each (lambda (p) (display p music-port)) parts)
    (newline music-port))
  (define (music-node! m depth seen)
    (let ((name (ly:music-property m 'name))
          (origin (music-origin (ly:music-property m 'origin #f))))
      (if (hashq-ref seen m)
          (music-line depth "= " name " " origin)
          (begin
            (hashq-set! seen m #t)
            (music-line depth name " " origin)
            (for-each
             (lambda (p)
               (let ((key (car p)) (v (cdr p)))
                 (cond ((eq? key 'origin))
                       ((ly:music? v)
                        (music-line (1+ depth) key ":")
                        (music-node! v (+ depth 2) seen))
                       ((and (pair? v) (list? v) (every ly:music? v))
                        (music-line (1+ depth) key ":")
                        (for-each (lambda (x) (music-node! x (+ depth 2) seen)) v))
                       (else (music-line (1+ depth) key " = " (music-value v))))))
             (sort (ly:music-mutable-properties m)
                   (lambda (a b) (string<? (symbol->string (car a)) (symbol->string (car b))))))))))
  (define (music-book! book)
    (for-each (lambda (s)
                (when (and (ly:score? s) (not (ly:score-error? s)))
                  (set! music-scores (1+ music-scores))
                  (music-line 0 "score " music-scores)
                  (music-node! (ly:score-music s) 0 (make-hash-table))))
              (ly:book-scores book))
    (for-each music-book! (ly:book-book-parts book))
    (force-output music-port)))
#(define (toplevel-book-handler book) (music-book! book))
