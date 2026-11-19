%% The context definitions of $defaultlayout and $defaultmidi (N3, 04 §7),
%% as declarations-init.ly leaves them: engraver-init.ly's in its \layout
%% and performer-init.ly's in midi-init.ly's \midi. LilyPond 2.26.0 runs
%% this file as it is (contexts.mjs --capture), and so does the engine,
%% after its own init (crates/parser/examples/parse.rs); both
%% write the same text to the standard output, which contexts.mjs compares.
%%
%% Each definition its output definition's kind, its name and its place,
%% the file's base name, line and column; then one line a key of
%% ly:context-def-lookup, sorted by the definitions' names:
%%
%%   layout Staff engraver-init.ly:59:1
%%     context-name Staff
%%     aliases ()
%%     accepts (Voice CueVoice NullVoice)
%%     default-child Voice
%%     ...
%%     property-ops 21
%%       (assign localAlterations ())
%%
%% the property operations one a line, the newest first, as LilyPond holds
%% them. A value is written as `write` writes it, but procedures by their
%% names, hash tables and unpure-pure containers by their kinds and parts,
%% and Global's grob descriptions by the name of what they are.
\version "2.26.0"

#(define (context-defs-value v)
  (cond ((procedure? v)
         (let ((name (procedure-name v)))
           (if name
               (string-append "#<procedure " (symbol->string name) ">")
               "#<procedure>")))
        ((hash-table? v) "#<hash-table>")
        ((ly:unpure-pure-container? v)
         (string-append "#<unpure-pure-container "
                        (context-defs-value (ly:unpure-pure-container-unpure-part v)) " "
                        (context-defs-value (ly:unpure-pure-container-pure-part v)) ">"))
        ((pair? v)
         (let loop ((v v) (parts '()))
           (cond ((pair? v) (loop (cdr v) (cons (context-defs-value (car v)) parts)))
                 ((null? v) (string-append "(" (string-join (reverse parts) " ") ")"))
                 (else (string-append "(" (string-join (reverse parts) " ")
                                      " . " (context-defs-value v) ")")))))
        ((vector? v) (string-append "#" (context-defs-value (vector->list v))))
        (else (object->string v))))

#(define (context-defs-place def)
  (let* ((text (object->string def))
         (end (- (string-length text) 1)))
    (let loop ((at (- end 1)))
      (if (or (< at 0) (memv (string-ref text at) '(#\/ #\space)))
          (substring text (+ at 1) end)
          (loop (- at 1))))))

#(define (context-defs-dump kind odef)
  (define (line depth . parts)
    (display (make-string (* 2 depth) #\space))
    (for-each display parts)
    (newline))
  (for-each
   (lambda (entry)
     (let ((def (cdr entry)))
       (line 0 kind " " (car entry) " " (context-defs-place def))
       (for-each
        (lambda (key)
          (let ((v (ly:context-def-lookup def key)))
            (line 1 key " "
                  (if (and (eq? key 'grob-descriptions) (eq? v all-grob-descriptions))
                      "all-grob-descriptions"
                      (context-defs-value v)))))
        '(context-name aliases accepts default-child group-type consists
          description grob-descriptions))
       (let ((ops (ly:context-def-lookup def 'property-ops)))
         (line 1 "property-ops " (length ops))
         (for-each (lambda (op) (line 2 (context-defs-value op))) ops))))
   (sort (ly:output-find-context-def odef)
         (lambda (a b) (string<? (symbol->string (car a)) (symbol->string (car b)))))))

#(context-defs-dump 'layout $defaultlayout)
#(context-defs-dump 'midi $defaultmidi)
