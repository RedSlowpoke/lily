%% The references of part 3 of MS3 (progress.md, "How part 3's gate reads";
%% MS3 step 25): after page breaking, one tab-separated line per live grob of
%% each system, the system's all-elements in their order, as chapter 05's
%% census (c5.ly, 05 §2.1) counts them: a grob that suicided has no name left
%% and is not written. capture.ly includes it after grobs.ly, whose functions
%% it uses; capture.mjs sets LAYOUT_OUT per score. Nothing it asks changes a
%% page: the pages' stencils are made before page-post-process
%% (paper-book.cc:775-788), and so are the offsets and extents they are drawn
%% with.
%%
%%   type  cause  link  columns  context  engraver  system  page  x  y  x-extent  y-extent  stencil
%%
%% type … engraver   grobs.ly's six fields, which pair the line with its
%%                   grobs.tsv line: those of the grob's unbroken original
%%                   (ly:grob-original), so that each broken copy of an item
%%                   or a spanner has its original's; the context and the
%%                   engraver are those it was announced from, - for a grob
%%                   no engraver announced
%% system, page      the system's index and its page's, each counted from 0
%%                   across the file's books and book parts
%% x, y              its offset relative to the System
%%                   (ly:grob-relative-coordinate)
%% x-extent,         its extent on that axis relative to itself (ly:grob-extent
%% y-extent          grob grob axis), its two ends with a space between, or -
%%                   for an empty extent
%% stencil           1 if its stencil property is a stencil, else 0
%%
%% Numbers have the four decimals the SVG writes (output-svg.scm's ~4f), as
%% inexact numbers so that a 0 is 0.0000 and not 0.
%%
%% Each system's grobs follow a line of the system itself (MS3 step 60,
%% OPEN H9), which is in no system's all-elements: its six fields are the
%% System's line of grobs.tsv (the Score's context and Score_engraver),
%% so the systems pair with each other in order as a broken spanner's
%% copies do; then its index and its page's, as its grobs'; x and y its
%% place on its page (the line's X-offset and Y-offset, page.scm's, 0 if
%% unset); its extents relative to itself, and its stencil flag.
%%
%% It also writes the springs and rods between the columns (MS3 step 31,
%% D128), into COLUMNS_OUT (columns.tsv), as they are right after
%% ly:spacing-spanner::set-springs: the SpacingSpanner's springs-and-rods
%% is wrapped, and after each call of it each column of the root System's
%% columns has a line, each of its prebroken pieces one after it (left,
%% then right):
%%
%%   column  spacing-wishes  springs  rods
%%
%% column            type@columns@break: its grobs.tsv type and columns
%%                   (its when), those of its original for a piece, and its
%%                   break direction (ly:item-break-dir: -1, 0 or 1); a
%%                   column named in a spring or rod is named so too
%% spacing-wishes    each grob of its spacing-wishes, type@columns@context
%%                   as grobs.tsv has them
%% springs           each of its ideal-distances, the other column and the
%%                   spring's ideal distance, minimum distance, inverse
%%                   stretch and inverse compress strengths
%% rods              each of its minimum-distances, the other column and the
%%                   distance
%%
%% each list in the order the column holds it, its entries separated by |,
%% - for none. A Spring has no Scheme getters in LilyPond 2.26.0, so its four
%% numbers are read from the smob's memory (Guile's (system foreign)), as
%% the C++ class lays them out; a spring of known numbers is read first, and
%% the file fails if they do not come back. The wrapper gives the callback's
%% value and reads only object properties, so the pages are as they were.
%%
%% And the columns of each system's line (MS3 step 32, D129), into LINE_OUT
%% (line.tsv), after page breaking with layout.tsv: a line a column of the
%% system's columns, in their order,
%%
%%   system  column  x
%%
%% system            the system's index, as layout.tsv's
%% column            columns.tsv's name of the column: a piece, the line's
%%                   first column (break 1) and its last (-1)
%% x                 its offset relative to the system, layout.tsv's x
#(begin
  (define layout-port
    (let ((port (open-file (or (getenv "LAYOUT_OUT") "layout.tsv") "w")))
      (set-port-encoding! port "UTF-8")
      port))
  (define (layout-line . xs)
    (display (string-join xs "\t") layout-port)
    (newline layout-port))
  ;; grobs.ly's six fields of each grob announced to the Score, by grob, read
  ;; in the Score's finalize as grobs.ly reads them: by page breaking, the
  ;; unbroken original of a broken grob has suicided and has no properties left.
  (define layout-fields (make-hash-table))
  (define (layout-recorder context)
    (let ((heard '()))
      (make-engraver
       (acknowledgers
        ((grob-interface engraver grob source-engraver)
         (when (eq? (grob::name grob) 'SpacingSpanner) (columns-wrap! grob))
         (set! heard (cons (list grob (symbol->string (grob::name grob)) (grobs-cause grob) (grobs-link grob)
                                 (grobs-label (ly:translator-context source-engraver))
                                 (grobs-class source-engraver))
                           heard))))
       ((finalize engraver)
        (for-each
         (lambda (h)
           (hashq-set! layout-fields (car h)
                       (list (list-ref h 1) (list-ref h 2) (list-ref h 3) (grobs-columns (car h))
                             (list-ref h 4) (list-ref h 5))))
         heard)
        ;; The System's, as grobs.ly writes its line.
        (let ((system (ly:context-property context 'rootSystem)))
          (hashq-set! layout-fields system
                      (list "System" (grobs-cause system) (grobs-link system) (grobs-columns system)
                            (grobs-label context) "Score_engraver")))))))
  ;; The fields of g's original, or of g itself when no engraver announced it.
  (define (layout-fields-of g)
    (let loop ((o g))
      (or (hashq-ref layout-fields o)
          (let ((up (ly:grob-original o)))
            (if (eq? up o)
                (list (symbol->string (grob::name g)) (grobs-cause g) (grobs-link g) (grobs-columns g) "-" "-")
                (loop up))))))
  (define (layout-number x)
    (cond ((and (inf? x) (> x 0)) "+inf")
          ((inf? x) "-inf")
          (else (ly:format "~4f" (+ 0.0 x)))))
  (define (layout-extent e)
    (if (or (not (pair? e)) (interval-empty? e))
        "-"
        (string-append (layout-number (car e)) " " (layout-number (cdr e)))))
  ;; The line of the system sys, the line prob `line' holds it.
  (define (layout-system sys line sysno pageno)
    (apply layout-line
           (append (layout-fields-of sys)
                   (list (number->string sysno) (number->string pageno)
                         (layout-number (ly:prob-property line 'X-offset 0.0))
                         (layout-number (ly:prob-property line 'Y-offset 0.0))
                         (layout-extent (ly:grob-extent sys sys X))
                         (layout-extent (ly:grob-extent sys sys Y))
                         (if (ly:stencil? (ly:grob-property sys 'stencil)) "1" "0")))))
  (define (layout-grob g sys sysno pageno)
    (apply layout-line
           (append (layout-fields-of g)
                   (list (number->string sysno) (number->string pageno)
                         (layout-number (ly:grob-relative-coordinate g sys X))
                         (layout-number (ly:grob-relative-coordinate g sys Y))
                         (layout-extent (ly:grob-extent g g X))
                         (layout-extent (ly:grob-extent g g Y))
                         (if (ly:stencil? (ly:grob-property g 'stencil)) "1" "0")))))
  ;; columns.tsv (D128).
  (define columns-port
    (let ((port (open-file (or (getenv "COLUMNS_OUT") "columns.tsv") "w")))
      (set-port-encoding! port "UTF-8")
      port))
  ;; A Spring's ideal distance, minimum distance, inverse stretch and
  ;; inverse compress strengths: the first four doubles of the C++ object
  ;; the smob's data word points to.
  (define columns-spring-numbers
    (let ((bytes (@ (system foreign) pointer->bytevector))
          (address (@ (system foreign) scm->pointer))
          (pointer (@ (system foreign) make-pointer))
          (word (@ (rnrs bytevectors) bytevector-u64-native-ref))
          (double (@ (rnrs bytevectors) bytevector-ieee-double-native-ref)))
      (lambda (spring)
        (unless (ly:spring? spring) (error "layout.ly: not a spring:" spring))
        (let ((data (bytes (pointer (word (bytes (address spring) 16) 8)) 32)))
          (map (lambda (i) (double data (* 8 i))) '(0 1 2 3))))))
  (let ((known (ly:spring-set-inverse-stretch-strength!
                (ly:spring-set-inverse-compress-strength! (ly:make-spring 1.5 0.25) 7.0) 3.0)))
    (unless (equal? (columns-spring-numbers known) '(1.5 0.25 3.0 7.0))
      (error "layout.ly: a Spring's memory is not LilyPond 2.26.0's:" (columns-spring-numbers known))))
  (define (columns-list entries)
    (if (null? entries) "-" (string-join entries "|")))
  (define (columns-name col)
    (let ((fields (layout-fields-of col)))
      (string-append (list-ref fields 0) "@" (list-ref fields 3) "@" (number->string (ly:item-break-dir col)))))
  (define (columns-wish g)
    (let ((fields (layout-fields-of g)))
      (string-append (list-ref fields 0) "@" (list-ref fields 3) "@" (list-ref fields 4))))
  (define (columns-line col)
    (let ((wishes (ly:grob-object col 'spacing-wishes #f)))
      (display (string-join
                (list (columns-name col)
                      (columns-list (if (ly:grob-array? wishes) (map columns-wish (ly:grob-array->list wishes)) '()))
                      (columns-list (map (lambda (e)
                                           (string-join (cons (columns-name (cdr e)) (map layout-number (columns-spring-numbers (car e)))) " "))
                                         (ly:grob-object col 'ideal-distances '())))
                      (columns-list (map (lambda (e) (string-append (columns-name (car e)) " " (layout-number (cdr e))))
                                         (ly:grob-object col 'minimum-distances '()))))
                "\t")
               columns-port)
      (newline columns-port)))
  (define (columns-paper-column? g)
    (memq (grob::name g) '(PaperColumn NonMusicalPaperColumn)))
  (define (columns-write spanner)
    (let* ((sys (let up ((g spanner))
                  (let ((p (ly:grob-parent g Y))) (if (ly:grob? p) (up p) g))))
           (pieces (filter (lambda (g) (and (columns-paper-column? g) (not (eq? (ly:grob-original g) g))))
                           (ly:grob-array->list (ly:grob-object sys 'all-elements)))))
      (for-each
       (lambda (col)
         (columns-line col)
         (for-each columns-line
                   (sort (filter (lambda (p) (eq? (ly:grob-original p) col)) pieces)
                         (lambda (a b) (< (ly:item-break-dir a) (ly:item-break-dir b))))))
       (ly:grob-array->list (ly:grob-object sys 'columns)))
      (force-output columns-port)))
  (define (columns-wrap! grob)
    (let ((proc (ly:grob-property-data grob 'springs-and-rods)))
      (ly:grob-set-property! grob 'springs-and-rods
                             (lambda (g)
                               (let ((value (if (procedure? proc) (proc g) proc)))
                                 (columns-write g)
                                 value)))))
  ;; line.tsv (D129).
  (define line-port
    (let ((port (open-file (or (getenv "LINE_OUT") "line.tsv") "w")))
      (set-port-encoding! port "UTF-8")
      port))
  (define (line-write sys sysno)
    (for-each
     (lambda (col)
       (display (string-join (list (number->string sysno) (columns-name col)
                                   (layout-number (ly:grob-relative-coordinate col sys X)))
                             "\t")
                line-port)
       (newline line-port))
     (ly:grob-array->list (ly:grob-object sys 'columns))))
  (define layout-sysno 0)
  (define layout-pageno 0)
  (define (layout-pages paper pages)
    (for-each
     (lambda (page)
       (for-each
        (lambda (line)
          (let ((sys (ly:prob-property line 'system-grob #f)))
            (when (ly:grob? sys)
              (layout-system sys line layout-sysno layout-pageno)
              (for-each (lambda (g) (layout-grob g sys layout-sysno layout-pageno))
                        (filter grob::name (ly:grob-array->list (ly:grob-object sys 'all-elements))))
              (line-write sys layout-sysno)
              (set! layout-sysno (1+ layout-sysno)))))
        (ly:prob-property page 'lines '()))
       (set! layout-pageno (1+ layout-pageno)))
     pages)
    (force-output layout-port)
    (force-output line-port))
  ;; After c5dump.ly's page-post-process, which it keeps.
  (let ((post-process (ly:output-def-lookup $defaultpaper 'page-post-process #f)))
    (ly:output-def-set-variable! $defaultpaper 'page-post-process
     (lambda (paper pages)
       (when (procedure? post-process) (post-process paper pages))
       (layout-pages paper pages))))
  (define (layout-instrument! odef)
    (when (and (ly:output-def? odef) (eq? (ly:output-def-lookup odef 'output-def-kind #f) 'layout))
      (let ((def (ly:output-def-lookup odef 'Score #f)))
        (when (and (ly:context-def? def)
                   (not (memq layout-recorder (ly:context-def-lookup def 'consists))))
          (ly:output-def-set-variable!
           odef 'Score
           (ly:context-def-modify def (ly:make-context-mod (list (list 'consists layout-recorder)))))))))
  (define (layout-instrument-book! book)
    (for-each (lambda (score) (when (ly:score? score) (for-each layout-instrument! (ly:score-output-defs score))))
              (ly:book-scores book))
    (for-each layout-instrument-book! (ly:book-book-parts book)))
  (define layout-book-handler toplevel-book-handler)
  (set! toplevel-book-handler
        (lambda (book)
          (layout-instrument! $defaultlayout)
          (layout-instrument-book! book)
          (layout-book-handler book))))
