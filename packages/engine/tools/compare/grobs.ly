%% The references of part 2 of MS3 (progress.md, "How part 2's gate reads";
%% MS3 step 12): every grob announced to the Score's engravers in a \layout
%% translation, one tab-separated line each in the order the Score hears
%% them, and a last line for the Score's System, which is not announced
%% (score-engraver.cc:81). capture.ly includes it; capture.mjs sets GROBS_OUT
%% per score. An acknowledger changes no grob, so the pages stay the same.
%%
%%   type  cause  link  columns  context  engraver
%%
%% type      the grob's name (grob::name), read when it is announced
%% cause     its cause property: a stream event by its first class
%%           (<ev note-event>), a grob by its name (<NoteHead>), or -
%% link      the origin of the event Grob::origin reaches through grob
%%           causes (grob.cc:775-797), file:line:char:column as
%%           ly:input-file-line-char-column gives it, the file by its name
%%           only; - when there is none
%% columns   the moment of its paper column, main and grace parts as
%%           rationals with a space between: an Item's ly:item-get-column
%%           (a paper column's own), a Spanner's two ly:spanner-bound
%%           columns joined by ;; - for a column there is none of, and
%%           for a grob killed in the translation (Grob::suicide)
%% context   the context of the engraver it is announced from, named as
%%           trace.ly names contexts: Name#n, n counting the contexts of its
%%           name in the order they are made, and =id if it has one
%% engraver  that engraver's class as LilyPond prints it
%%
%% The System's line has the Score's context and Score_engraver, which makes
%% it, and ends each translation's lines. The columns are read in the
%% engraver's finalize, which comes after every other's: the Score's
%% children finalize first (score-engraver.cc:60-64, UP), and the engraver
%% is the Score's last, added when the book is handled. Line breaking has
%% not yet run, so no grob has lost its parents to Grob::suicide.
%%
%% Every context definition of a layout gets a labeling engraver, as
%% trace.ly's recorder, so that contexts are numbered in the order they are
%% made whether or not they announce grobs; the Score's also gets the
%% recording one.
#(begin
  (define grobs-port
    (let ((port (open-file (or (getenv "GROBS_OUT") "grobs.tsv") "w")))
      (set-port-encoding! port "UTF-8")
      port))
  (define (grobs-line . xs)
    (display (string-join xs "\t") grobs-port)
    (newline grobs-port))
  (define grobs-labels (make-hash-table))
  (define grobs-label-counts (make-hash-table))
  (define (grobs-label ctx)
    (or (hashq-ref grobs-labels ctx)
        (let* ((name (ly:context-name ctx))
               (n (hashq-ref grobs-label-counts name 0))
               (id (ly:context-id ctx))
               (l (format #f "~a#~a~a" name n (if (and (string? id) (not (string-null? id))) (string-append "=" id) ""))))
          (hashq-set! grobs-label-counts name (1+ n))
          (hashq-set! grobs-labels ctx l)
          l)))
  (define (grobs-labeler context)
    (make-engraver ((initialize engraver) (grobs-label context))))
  (define (grobs-cause grob)
    (let ((cause (ly:grob-property grob 'cause)))
      (cond ((ly:stream-event? cause) (format #f "<ev ~a>" (car (ly:event-property cause 'class))))
            ((ly:grob? cause) (format #f "<~a>" (grob::name cause)))
            ((or (null? cause) (not cause)) "-")
            (else (string-map (lambda (c) (if (memv c '(#\tab #\newline)) #\space c))
                              (object->string cause))))))
  (define (grobs-link grob)
    (let* ((cause (let loop ((cause (ly:grob-property grob 'cause)))
                    (if (ly:grob? cause) (loop (ly:grob-property cause 'cause)) cause)))
           (origin (and (ly:stream-event? cause) (ly:event-property cause 'origin #f))))
      (if (ly:input-location? origin)
          (let ((l (ly:input-file-line-char-column origin)))
            (format #f "~a:~a:~a:~a" (basename (car l)) (cadr l) (caddr l) (cadddr l)))
          "-")))
  (define (grobs-moment item)
    (let* ((col (and (ly:item? item) (ly:item-get-column item)))
           (when (and (ly:grob? col) (ly:grob-property col 'when #f))))
      (if (ly:moment? when)
          (format #f "~a ~a" (ly:moment-main when) (ly:moment-grace when))
          "-")))
  ;; A grob killed in the translation (Grob::suicide) has no name, parents
  ;; or bounds left, and so no column.
  (define (grobs-columns grob)
    (cond ((not (grob::name grob)) "-")
          ((ly:spanner? grob)
           (string-append (grobs-moment (ly:spanner-bound grob LEFT)) ";"
                          (grobs-moment (ly:spanner-bound grob RIGHT))))
          (else (grobs-moment grob))))
  (define (grobs-class translator)
    (let ((s (format #f "~a" translator)))
      (string-trim-both (string-drop (string-drop-right s 1) (string-length "#<Translator ")))))
  (define (grobs-recorder context)
    (let ((heard '()))
      (make-engraver
       (acknowledgers
        ((grob-interface engraver grob source-engraver)
         (set! heard (cons (list grob (symbol->string (grob::name grob)) (grobs-cause grob) (grobs-link grob)
                                 (grobs-label (ly:translator-context source-engraver))
                                 (grobs-class source-engraver))
                           heard))))
       ((finalize engraver)
        (for-each
         (lambda (h)
           (grobs-line (list-ref h 1) (list-ref h 2) (list-ref h 3)
                       (grobs-columns (car h)) (list-ref h 4) (list-ref h 5)))
         (reverse heard))
        (let ((system (ly:context-property context 'rootSystem)))
          (grobs-line "System" (grobs-cause system) (grobs-link system) (grobs-columns system)
                      (grobs-label context) "Score_engraver"))
        (force-output grobs-port)))))
  (define (grobs-instrument! odef)
    (when (and (ly:output-def? odef) (eq? (ly:output-def-lookup odef 'output-def-kind #f) 'layout))
      (for-each
       (lambda (entry)
         (let ((def (cdr entry)))
           (when (and (ly:context-def? def)
                      (not (memq grobs-labeler (ly:context-def-lookup def 'consists))))
             (ly:output-def-set-variable!
              odef (car entry)
              (ly:context-def-modify
               def (ly:make-context-mod
                    (cons (list 'consists grobs-labeler)
                          (if (eq? (ly:context-def-lookup def 'context-name) 'Score)
                              (list (list 'consists grobs-recorder))
                              '()))))))))
       (ly:module->alist (ly:output-def-scope odef)))))
  (define (grobs-instrument-book! book)
    (for-each (lambda (score) (when (ly:score? score) (for-each grobs-instrument! (ly:score-output-defs score))))
              (ly:book-scores book))
    (for-each grobs-instrument-book! (ly:book-book-parts book)))
  (define grobs-book-handler toplevel-book-handler)
  (set! toplevel-book-handler
        (lambda (book)
          (grobs-instrument! $defaultlayout)
          (grobs-instrument-book! book)
          (grobs-book-handler book))))
