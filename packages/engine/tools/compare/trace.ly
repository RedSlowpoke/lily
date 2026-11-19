%% The translation trace (04 Appendix A's c4.ly in trace mode, MS2 step 22):
%% a settings file that records, in every \layout translation, what each
%% context hears per timestep and its own properties at each bar start, for
%% trace.mjs to compare with the engine's. TRACE_OUT names the file, which
%% is appended to. Each line is tab-separated:
%%
%%   P  pass kind                      a translation starts (kind: layout)
%%   C  main grace context parent      a context is made, below its parent
%%   E  main grace context class args  an event, by its deepest hearer
%%   X  main grace context             a context is finalized
%%   S  main grace context prop value  a property the context itself holds,
%%                                     at a timestep where the Score's
%%                                     measurePosition is zero
%%
%% A context is `Name#n`, n counting the contexts of its name in the order
%% they are made, and `=id` after it if it has one. `E` lines are written
%% when the timestep ends, in the order the events were first heard; an
%% event is the deepest context's that heard it, which is where it was sent.
%% `S` lines go down the tree from the Score, one property a line, sorted
%% by name; the properties are define-context-properties.scm's but those
%% that hold grobs (c4.ly's list). Grobs (c4.ly's `G` lines) and overrides
%% are not recorded: part 3 of MS2 makes no grobs.
%%
%% A value is written as `write` writes it, but a procedure as `<p>`
%% (Guile's procedure-name is slow on some), a context as `<label>`, a grob
%% as `<Name>`, music as `<m Name>`, a stream event as `<ev class>`, an
%% input location as `<loc>`, a grob array as `<ga n>` and a hash table as
%% `<hash>`. A moment's main and grace parts are rationals.
%%
%% A listener added by hand to a context's dispatcher outlives the context
%% and crashes LilyPond (04 Appendix A), so every context consists of a
%% recording engraver instead, added to each definition of the layouts a
%% book is handled with.
#(begin
  (use-modules (ice-9 format))
  (define trace-port (open-file (or (getenv "TRACE_OUT") "trace.tsv") "a"))
  (define (trace-line . xs)
    (display (string-join (map (lambda (x) (if (string? x) x (format #f "~a" x))) xs) "\t") trace-port)
    (newline trace-port))
  (define trace-pass 0)
  (define trace-labels (make-hash-table))
  (define trace-label-counts (make-hash-table))
  (define (trace-label ctx)
    (or (hashq-ref trace-labels ctx)
        (let* ((name (ly:context-name ctx))
               (n (hashq-ref trace-label-counts name 0))
               (id (ly:context-id ctx))
               (l (format #f "~a#~a~a" name n (if (and (string? id) (not (string-null? id))) (string-append "=" id) ""))))
          (hashq-set! trace-label-counts name (1+ n))
          (hashq-set! trace-labels ctx l)
          l)))
  (define (trace-value v)
    (cond ((procedure? v) "<p>")
          ((ly:grob? v) (format #f "<~a>" (grob::name v)))
          ((ly:context? v) (format #f "<~a>" (trace-label v)))
          ((ly:stream-event? v) (format #f "<ev ~a>" (car (ly:event-property v 'class))))
          ((ly:music? v) (format #f "<m ~a>" (ly:music-property v 'name)))
          ((ly:input-location? v) "<loc>")
          ((ly:grob-array? v) (format #f "<ga ~a>" (ly:grob-array-length v)))
          ((hash-table? v) "<hash>")
          ((pair? v)
           (let loop ((v v) (parts '()))
             (cond ((pair? v) (loop (cdr v) (cons (trace-value (car v)) parts)))
                   ((null? v) (string-append "(" (string-join (reverse parts) " ") ")"))
                   (else (string-append "(" (string-join (reverse parts) " ") " . " (trace-value v) ")")))))
          ((vector? v) (string-append "#" (trace-value (vector->list v))))
          (else (let ((s (object->string v)))
                  ;; Tabs and newlines would split a line.
                  (string-map (lambda (c) (if (memv c '(#\tab #\newline)) #\space c)) s)))))
  (define (trace-moment ctx)
    (let ((now (ly:context-current-moment ctx)))
      (list (ly:moment-main now) (ly:moment-grace now))))
  (define (trace-event-args ev)
    (string-join
     (map (lambda (key) (trace-value (ly:event-property ev key '())))
          (case (car (ly:event-property ev 'class))
            ((SetProperty UnsetProperty) '(symbol value once))
            ((Override Revert) '(symbol property-path value once))
            (else '(pitch duration drum-type text span-direction articulation-type tremolo-type alteration figure))))
     " "))
  (define trace-score #f)
  (define trace-heard (make-hash-table))
  (define trace-heard-order '())
  (define (trace-depth ctx) (let loop ((c ctx) (n 0)) (if c (loop (ly:context-parent c) (1+ n)) n)))
  (define (trace-heard! ctx ev)
    (let ((old (hashq-ref trace-heard ev)))
      (unless old (set! trace-heard-order (cons ev trace-heard-order)))
      (when (or (not old) (> (trace-depth ctx) (trace-depth old)))
        (hashq-set! trace-heard ev ctx))))
  (define (trace-flush-events!)
    (for-each (lambda (ev)
                (apply trace-line "E" (append (trace-moment trace-score)
                                              (list (trace-label (hashq-ref trace-heard ev))
                                                    (car (ly:event-property ev 'class))
                                                    (trace-event-args ev)))))
              (reverse trace-heard-order))
    (set! trace-heard-order '())
    (hash-clear! trace-heard))
  (define (trace-context-recorder context)
    (make-engraver
     ((initialize engraver)
      (let ((parent (ly:context-parent context)))
        ;; The Score, made below Global, starts a translation.
        (when (and parent (not (ly:context-parent parent)))
          (set! trace-pass (1+ trace-pass))
          (trace-line "P" trace-pass 'layout)
          (set! trace-score context))
        (apply trace-line "C" (append (trace-moment context)
                                      (list (trace-label context)
                                            (if parent (trace-label parent) "-"))))))
     ((finalize engraver)
      (apply trace-line "X" (append (trace-moment context) (list (trace-label context)))))
     (listeners
      ((music-event engraver event) (trace-heard! context event))
      ((SetProperty engraver event) (trace-heard! context event))
      ((UnsetProperty engraver event) (trace-heard! context event))
      ((Override engraver event) (trace-heard! context event))
      ((Revert engraver event) (trace-heard! context event)))))
  ;; Every context definition of a layout gets the recorder when the book
  ;; is handled, so that those a score's \layout defines from scratch
  ;; (T29's MarkLine) have it too.
  (define (trace-instrument! odef)
    (when (and (ly:output-def? odef) (eq? (ly:output-def-lookup odef 'output-def-kind #f) 'layout))
      (for-each
       (lambda (entry)
         (let ((def (cdr entry)))
           (when (and (ly:context-def? def)
                      (not (memq trace-context-recorder (ly:context-def-lookup def 'consists))))
             (ly:output-def-set-variable!
              odef (car entry)
              (ly:context-def-modify def (ly:make-context-mod (list (list 'consists trace-context-recorder))))))))
       (ly:module->alist (ly:output-def-scope odef)))))
  (define (trace-instrument-book! book)
    (for-each (lambda (score) (when (ly:score? score) (for-each trace-instrument! (ly:score-output-defs score))))
              (ly:book-scores book))
    (for-each trace-instrument-book! (ly:book-book-parts book)))
  (define trace-book-handler toplevel-book-handler)
  (set! toplevel-book-handler
        (lambda (book)
          (trace-instrument! $defaultlayout)
          (trace-instrument-book! book)
          (trace-book-handler book)))
  (define (trace-contexts ctx)
    (cons ctx (append-map trace-contexts (ly:context-children ctx))))
  (define trace-properties
    (sort (filter (lambda (s) (not (memq s '(currentCommandColumn currentMusicalColumn currentBarLine
                                             busyGrobs stavesFound breakableSeparationItem
                                             hasStaffSpacing measureStartNow rootSystem
                                             lastChord))))
                  all-translation-properties)
          (lambda (a b) (string<? (symbol->string a) (symbol->string b)))))
  (define (trace-state! score)
    (for-each
     (lambda (ctx)
       (for-each
        (lambda (sym)
          (when (eq? (ly:context-property-where-defined ctx sym #f) ctx)
            (apply trace-line "S" (append (trace-moment ctx)
                                          (list (trace-label ctx) sym
                                                (trace-value (ly:context-property ctx sym)))))))
        trace-properties))
     (trace-contexts score)))
  (define (trace-recorder context)
    (make-translator
     ((stop-translation-timestep translator)
      (trace-flush-events!)
      (let ((pos (ly:context-property context 'measurePosition #f)))
        (when (and (ly:moment? pos) (zero? (ly:moment-main pos)) (zero? (ly:moment-grace pos)))
          (trace-state! context))))
     ((finalize translator)
      (force-output trace-port)))))

\layout { \context { \Score \consists #trace-recorder } }
