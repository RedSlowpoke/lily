%% L1 parts 1 and 2's capture (01 §3.5, MS3 steps 13 to 15 and 21): calls on the
%% grobs that fixtures/grobs.ly's Scheme engravers make, run in LilyPond
%% 2.26.0, one record a call, as part B's reference for crates/layout's
%% grobs:
%%
%%   node tools/compare/capture.mjs grob-calls    (into <corpus dir>/ref/grob-calls/)
%%   lilypond tools/compare/grob-calls.ly         (grob-calls.txt in the current directory)
%%
%% The file includes the fixture unchanged, so its grobs and their links are
%% the fixture's, and adds a recorder to the Score that hears the fixture's
%% grobs (Fingering, StringNumber, StrokeFinger) as the Score's engravers
%% acknowledge them, in grobs.tsv's order, and runs the calls in its
%% finalize, after every other engraver's (score-engraver.cc:60-64), when
%% each grob has its layout. In the calls (fingering n), (string-number n)
%% and (stroke-finger n) are the nth grob of that type heard; the first
%% timestep's are fingering 0 (c'4), stroke-finger 0 (no cause),
%% string-number 0 and stroke-finger 1 (of it).
%%
%% Each record is a line: the call as Guile writes it, a tab, and its value
%% as grob-calls-show writes it: a grob as (grob <type> <n>), a stream event
%% as (event <first class>), a stencil as (stencil <expression> <x extent>
%% <y extent>), a grob array as (grob-array <grob> …) (MS3 step 14), a
%% procedure as procedure, an unpure-pure container as
%% unpure-pure-container, nothing as unspecified, an error as (error <key>).
%% GROB_CALLS_OUT names the file (default grob-calls.txt).
%%
%% The calls, in their order:
%% - properties with no callback: set on the grob, then its definition's
%%   (define-grobs.scm), the default of ly:grob-property, a set '(), cause;
%% - callbacks: a procedure called once and its value kept, one reading
%%   another property, one returning a procedure, unpure-pure containers
%%   (their unpure part), one that throws and leaves calculation-in-progress;
%% - chains: output-lib.scm's grob::compose-function and
%%   grob::offset-function over plain data, a procedure, a container and a
%%   definition's data, and over each other, as grob-closure.cc chains them;
%% - cycles: a property reading itself and two reading each other, read
%%   twice: each read writes grob-property.cc's programming error;
%% - parents, offsets and extents on both axes among the first timestep's
%%   grobs, whose stencils the calls set: relative coordinates, extents
%%   with minimum extents and an explicit X-extent, common refpoints,
%%   translate-axis before and after an offset is computed, and
%%   ly:grob::x-parent-positioning;
%% - the grob's warnings at Grob::origin (grob.cc:775-797): an infinite
%%   offset's on a grob of an event, of a grob of an event and of no cause,
%%   and translate-axis by NaN;
%% - suicide: what a dead grob keeps;
%% - print stencils: each grob's link as output-svg.scm's grob-cause makes
%%   it (output-svg.scm:440-466), and whiteout, color, transparent,
%%   rotation and output-attributes set for the page (grob.cc:131-236);
%% - MS3 step 14's, after step 13's 163: interfaces and items, grob arrays,
%%   pointer groups and units, pure and unpure calls on grobs, and the rest
%%   of grob-scheme.cc's, on properties the page does not read, so that the
%%   page stays step 13's. MS3 step 14a's three: a grob's default font,
%%   through its layout's parent. MS3 step 15's: a grob's column, system
%%   and rank, through its PaperColumn. MS3 step 21's: a grob's vertical
%%   order, through the VerticalAlignment.
%%
%% Before the calls each grob gets a constant stencil, so that no call and
%% no page needs a font; the grobs whose offsets the calls make infinite
%% die afterwards, so that the page has none. The calls' warnings go to
%% stderr, LilyPond's own, in the calls' order.
\version "2.26.0"

#(begin
  (define grob-calls-heard '())
  (define grob-calls-module (current-module))
  (define grob-calls-count 0)
  (define grob-calls-positioning 0)
  (define (grob-calls-nth type n)
    (car (list-ref (filter (lambda (h) (eq? (cdr h) type)) grob-calls-heard) n)))
  (define (fingering n) (grob-calls-nth 'Fingering n))
  (define (string-number n) (grob-calls-nth 'StringNumber n))
  (define (stroke-finger n) (grob-calls-nth 'StrokeFinger n))
  ;; A property's data in a grob's definition.
  (define (grob-calls-definition type property)
    (assq-ref (assq-ref all-grob-descriptions type) property))
  ;; output-svg.scm's grob-cause, the link of a grob's print stencil; its
  ;; flag for the page's writer is given back as it was.
  (define (grob-calls-link grob)
    (let* ((svg (resolve-module '(lily output-svg)))
           (flag (module-ref svg 'have-grob-cause?))
           (link ((module-ref svg 'grob-cause) '(0 . 0) grob)))
      (module-set! svg 'have-grob-cause? flag)
      link))
  (define (grob-calls-show x)
    (cond ((ly:grob? x)
           (let ((h (assq x grob-calls-heard)))
             (if h
                 (list 'grob (cdr h)
                       (let count ((l grob-calls-heard) (n 0))
                         (cond ((eq? (car l) h) n)
                               ((eq? (cdar l) (cdr h)) (count (cdr l) (1+ n)))
                               (else (count (cdr l) n)))))
                 (list 'grob (grob::name x)))))
          ((ly:stream-event? x) (list 'event (car (ly:event-property x 'class))))
          ((ly:stencil? x)
           (list 'stencil (ly:stencil-expr x) (ly:stencil-extent x X) (ly:stencil-extent x Y)))
          ((ly:grob-array? x) (cons 'grob-array (map grob-calls-show (ly:grob-array->list x))))
          ((ly:unpure-pure-container? x) 'unpure-pure-container)
          ((procedure? x) 'procedure)
          ((unspecified? x) 'unspecified)
          ((pair? x) (cons (grob-calls-show (car x)) (grob-calls-show (cdr x))))
          ((vector? x) (list->vector (map grob-calls-show (vector->list x))))
          (else x)))
  (define (grob-calls-run port call)
    (write call port)
    (display "\t" port)
    (write (catch #t
             (lambda () (grob-calls-show (eval call grob-calls-module)))
             (lambda (key . args) (list 'error key)))
           port)
    (newline port)
    (force-output port)
    (force-output (current-error-port)))
  (define grob-calls
    '(
      ;; Properties with no callback: set on the grob, then its definition's.
      (ly:grob-property (fingering 0) 'text)
      (ly:grob-property (fingering 0) 'direction)
      (ly:grob-property (fingering 0) 'font-size)
      (ly:grob-property (stroke-finger 0) 'font-shape)
      (ly:grob-property (stroke-finger 0) 'digit-names)
      (ly:grob-property (string-number 0) 'number-type)
      (ly:grob-property (fingering 0) 'no-such-property)
      (ly:grob-property (fingering 0) 'no-such-property 'fallback)
      (ly:grob-property (fingering 0) 'padding 'fallback)
      (ly:grob-set-property! (fingering 0) 'padding 1.25)
      (ly:grob-property (fingering 0) 'padding)
      (ly:grob-set-property! (fingering 0) 'staff-padding '())
      (ly:grob-property (fingering 0) 'staff-padding 'fallback)
      (ly:grob-property (fingering 0) 'cause)
      (ly:grob-property (string-number 0) 'cause)
      (ly:grob-property (stroke-finger 1) 'cause)
      (ly:grob-property (stroke-finger 0) 'cause)
      (grob::name (stroke-finger 1))
      ;; Callbacks: called once, the value kept in the grob.
      (ly:grob-set-property! (fingering 1) 'X-offset (lambda (grob) 0.75))
      (ly:grob-property (fingering 1) 'X-offset)
      (ly:grob-set-property! (fingering 1) 'Y-offset
                             (lambda (grob) (set! grob-calls-count (1+ grob-calls-count)) -0.5))
      (ly:grob-property (fingering 1) 'Y-offset)
      (ly:grob-property (fingering 1) 'Y-offset)
      grob-calls-count
      (ly:grob-set-property! (fingering 1) 'extra-offset
                             (lambda (grob) (cons (ly:grob-property grob 'X-offset) (ly:grob-property grob 'padding))))
      (ly:grob-property (fingering 1) 'extra-offset)
      (ly:grob-set-property! (fingering 1) 'staff-padding (lambda (grob) (lambda (grob) 2.5)))
      (ly:grob-property (fingering 1) 'staff-padding)
      (ly:grob-property (fingering 1) 'staff-padding)
      (ly:grob-set-property! (fingering 1) 'minimum-X-extent
                             (ly:make-unpure-pure-container (lambda (grob) '(0 . 2)) (lambda (grob start end) '(0 . 4))))
      (ly:grob-property (fingering 1) 'minimum-X-extent)
      (ly:grob-set-property! (fingering 1) 'minimum-Y-extent (ly:make-unpure-pure-container '(1 . 3)))
      (ly:grob-property (fingering 1) 'minimum-Y-extent)
      (ly:grob-property (fingering 1) 'minimum-Y-extent)
      (ly:grob-set-property! (fingering 1) 'slur-padding (lambda (grob) (throw 'grob-calls-thrown)))
      (ly:grob-property (fingering 1) 'slur-padding)
      (ly:grob-property (fingering 1) 'slur-padding)
      (ly:grob-set-property! (fingering 1) 'slur-padding 0.2)
      (ly:grob-property (fingering 1) 'slur-padding)
      ;; Chains: output-lib.scm's two callbacks, as grob-closure.cc's
      ;; chain_callback and add_offset_callback make them.
      (grob::compose-function (lambda (grob value) (* 2 value)) 3)
      (ly:grob-set-property! (fingering 2) 'padding (grob::compose-function (lambda (grob value) (* 2 value)) 3))
      (ly:grob-property (fingering 2) 'padding)
      (ly:grob-set-property! (fingering 2) 'staff-padding
                             (grob::compose-function (lambda (grob value) (+ value 1))
                                                     (grob::compose-function (lambda (grob value) (* value 10))
                                                                             (lambda (grob) 0.25))))
      (ly:grob-property (fingering 2) 'staff-padding)
      (grob::compose-function (ly:make-unpure-pure-container (lambda (grob value) (+ value 1))) 1)
      (ly:grob-set-property! (fingering 2) 'X-padding
                             (grob::compose-function (ly:make-unpure-pure-container (lambda (grob value) (+ value 1))
                                                                                    (lambda (grob start end value) value))
                                                     1))
      (ly:grob-property (fingering 2) 'X-padding)
      (grob-calls-definition 'Fingering 'Y-extent)
      (ly:grob-set-property! (fingering 2) 'Y-extent
                             (grob::compose-function (lambda (grob extent) (cons (car extent) (+ 1 (cdr extent))))
                                                     (grob-calls-definition 'Fingering 'Y-extent)))
      (ly:grob-property (fingering 2) 'Y-extent)
      (grob::offset-function (lambda (grob) 1) '())
      (grob::offset-function (lambda (grob) 1) #f)
      (ly:grob-set-property! (fingering 2) 'X-offset (grob::offset-function (lambda (grob) 1) 0.25))
      (ly:grob-property (fingering 2) 'X-offset)
      (ly:grob-set-property! (fingering 2) 'Y-offset
                             (grob::offset-function (lambda (grob) 1)
                                                    (grob::offset-function (lambda (grob) 0.5) (lambda (grob) 2))))
      (ly:grob-property (fingering 2) 'Y-offset)
      (ly:grob-set-property! (string-number 2) 'X-offset (grob::offset-function (lambda (grob) 3) 2 *))
      (ly:grob-property (string-number 2) 'X-offset)
      (ly:grob-set-property! (string-number 2) 'Y-offset (grob::offset-function (lambda (grob) 1) (lambda (grob) '())))
      (ly:grob-property (string-number 2) 'Y-offset)
      (ly:grob-set-property! (string-number 2) 'padding
                             (grob::offset-function (lambda (grob) 1)
                                                    (ly:make-unpure-pure-container (lambda (grob) 2) (lambda (grob start end) 3))))
      (ly:grob-property (string-number 2) 'padding)
      ;; Cycles: each read of calculation-in-progress is a programming error.
      (ly:grob-set-property! (fingering 3) 'X-offset (lambda (grob) (ly:grob-property grob 'X-offset)))
      (ly:grob-property (fingering 3) 'X-offset)
      (ly:grob-property (fingering 3) 'X-offset)
      (ly:grob-set-property! (fingering 3) 'X-offset 0)
      (ly:grob-set-property! (string-number 3) 'X-offset (lambda (grob) (ly:grob-property grob 'Y-offset)))
      (ly:grob-set-property! (string-number 3) 'Y-offset (lambda (grob) (ly:grob-property grob 'X-offset)))
      (ly:grob-property (string-number 3) 'X-offset)
      (ly:grob-property (string-number 3) 'Y-offset)
      (ly:grob-set-property! (string-number 3) 'X-offset 0)
      (ly:grob-set-property! (string-number 3) 'Y-offset 0)
      ;; Parents, offsets and extents: string-number 0 and stroke-finger 0
      ;; hang from fingering 0, stroke-finger 1 from string-number 0.
      (ly:grob-set-parent! (string-number 0) X (fingering 0))
      (ly:grob-set-parent! (string-number 0) Y (fingering 0))
      (ly:grob-set-parent! (stroke-finger 1) X (string-number 0))
      (ly:grob-set-parent! (stroke-finger 1) Y (string-number 0))
      (ly:grob-set-parent! (stroke-finger 0) X (fingering 0))
      (ly:grob-set-parent! (stroke-finger 0) Y (fingering 0))
      (ly:grob-set-property! (fingering 0) 'stencil (ly:make-stencil '(draw-line 0.1 0 0 1 2) '(0 . 1) '(0 . 2)))
      (ly:grob-set-property! (string-number 0) 'stencil
                             (ly:make-stencil '(round-filled-box 0.5 0.5 0 1 0.1) '(-0.5 . 0.5) '(0 . 1)))
      (ly:grob-set-property! (stroke-finger 1) 'stencil
                             (lambda (grob) (ly:make-stencil '(circle 0.5 0.1 #t) '(-0.5 . 0.5) '(-0.5 . 0.5))))
      (ly:grob-set-property! (stroke-finger 0) 'stencil '())
      (ly:grob-set-property! (string-number 0) 'X-offset 1.5)
      (ly:grob-set-property! (string-number 0) 'Y-offset 2)
      (ly:grob-set-property! (stroke-finger 1) 'X-offset (grob::offset-function (lambda (grob) 0.25) 0.5))
      (ly:grob-set-property! (stroke-finger 1) 'Y-offset
                             (lambda (grob)
                               (let ((parent (ly:grob-parent grob Y)))
                                 (cdr (ly:grob-extent parent parent Y)))))
      (ly:grob-set-property! (stroke-finger 1) 'minimum-Y-extent '(-2 . 0))
      (ly:grob-set-property! (fingering 0) 'positioning-done
                             (lambda (grob) (set! grob-calls-positioning (1+ grob-calls-positioning)) #t))
      (ly:grob-set-property! (stroke-finger 0) 'X-offset ly:grob::x-parent-positioning)
      (ly:grob-property (fingering 0) 'X-extent)
      (ly:grob-property (fingering 0) 'Y-extent)
      (ly:grob-extent (fingering 0) (fingering 0) X)
      (ly:grob-extent (fingering 0) (fingering 0) Y)
      (ly:grob-relative-coordinate (fingering 0) (fingering 0) X)
      (ly:grob-relative-coordinate (string-number 0) (fingering 0) X)
      (ly:grob-relative-coordinate (stroke-finger 1) (fingering 0) X)
      (ly:grob-relative-coordinate (stroke-finger 1) (fingering 0) Y)
      (ly:grob-relative-coordinate (stroke-finger 1) (string-number 0) Y)
      (ly:grob-extent (string-number 0) (fingering 0) X)
      (ly:grob-extent (string-number 0) (fingering 0) Y)
      (ly:grob-extent (stroke-finger 1) (fingering 0) X)
      (ly:grob-extent (stroke-finger 1) (fingering 0) Y)
      (ly:grob-extent (stroke-finger 1) (string-number 0) Y)
      (ly:grob-set-property! (string-number 0) 'minimum-X-extent '(-2 . -1))
      (ly:grob-extent (string-number 0) (fingering 0) X)
      (ly:grob-relative-coordinate (stroke-finger 0) (fingering 0) X)
      grob-calls-positioning
      (ly:grob-extent (stroke-finger 0) (fingering 0) X)
      (ly:grob-property (stroke-finger 0) 'X-offset)
      (ly:grob-common-refpoint (stroke-finger 1) (stroke-finger 0) X)
      (ly:grob-common-refpoint (stroke-finger 1) (string-number 0) Y)
      (ly:grob-common-refpoint (string-number 0) (stroke-finger 1) X)
      (ly:grob-common-refpoint (fingering 0) (fingering 0) Y)
      (ly:grob-translate-axis! (string-number 0) 0.5 X)
      (ly:grob-relative-coordinate (stroke-finger 1) (fingering 0) X)
      (ly:grob-extent (stroke-finger 1) (fingering 0) X)
      (ly:grob-set-parent! (stroke-finger 2) X (string-number 1))
      (ly:grob-set-property! (string-number 1) 'X-extent '(0 . 3))
      (ly:grob-set-property! (stroke-finger 2) 'X-offset
                             (lambda (grob) (set! grob-calls-count (1+ grob-calls-count)) 10))
      (ly:grob-translate-axis! (stroke-finger 2) 1 X)
      (ly:grob-relative-coordinate (stroke-finger 2) (string-number 1) X)
      grob-calls-count
      (ly:grob-property (stroke-finger 2) 'X-offset)
      (ly:grob-extent (string-number 1) (string-number 1) X)
      ;; The grob's warnings, located at its origin: of an event, of a grob
      ;; of an event, of no cause; translate-axis by NaN.
      (ly:grob-set-parent! (fingering 4) Y (fingering 0))
      (ly:grob-set-property! (fingering 4) 'Y-offset +inf.0)
      (ly:grob-extent (fingering 4) (fingering 0) Y)
      (ly:grob-set-parent! (string-number 4) X (fingering 0))
      (ly:grob-set-property! (string-number 4) 'X-offset -inf.0)
      (ly:grob-extent (string-number 4) (fingering 0) X)
      (ly:grob-set-property! (stroke-finger 0) 'Y-offset +inf.0)
      (ly:grob-extent (stroke-finger 0) (fingering 0) Y)
      (ly:grob-translate-axis! (fingering 5) +nan.0 X)
      (ly:grob-relative-coordinate (fingering 5) (fingering 5) X)
      ;; Print stencils: each grob's link, as the page's writer makes it.
      (grob-calls-link (fingering 0))
      (grob-calls-link (fingering 1))
      (grob-calls-link (fingering 2))
      (grob-calls-link (fingering 3))
      (grob-calls-link (fingering 4))
      (grob-calls-link (fingering 5))
      (grob-calls-link (string-number 0))
      (grob-calls-link (stroke-finger 0))
      (grob-calls-link (stroke-finger 1))
      ;; Suicide: the cause is kept and nothing else; a dead grob takes no
      ;; property. The grobs of infinite offsets die.
      (ly:grob-suicide! (fingering 4))
      (ly:grob-property (fingering 4) 'cause)
      (ly:grob-property (fingering 4) 'text)
      (grob::name (fingering 4))
      (ly:grob-set-property! (fingering 4) 'text "4")
      (ly:grob-property (fingering 4) 'text)
      (grob-calls-link (fingering 4))
      (ly:grob-suicide! (string-number 4))
      (ly:grob-suicide! (stroke-finger 0))
      (ly:grob-property (stroke-finger 0) 'cause)
      ;; The page's print stencils: whiteout, then grob-cause, rotation,
      ;; color and output-attributes; a transparent grob's has no link.
      (ly:grob-set-property! (fingering 0) 'whiteout #t)
      (ly:grob-set-property! (fingering 0) 'color "red")
      (ly:grob-set-property! (string-number 0) 'whiteout 2)
      (ly:grob-set-property! (string-number 0) 'whiteout-style 'rounded-box)
      (ly:grob-set-property! (string-number 0) 'whiteout-color '(0 0 1))
      (ly:grob-set-property! (string-number 0) 'color '(0 0.5 0))
      (ly:grob-set-property! (stroke-finger 1) 'transparent #t)
      (ly:grob-set-property! (fingering 1) 'rotation '(30 0 0))
      (ly:grob-set-property! (fingering 1) 'color '(0.2 0.4 0.6 0.5))
      (ly:grob-set-property! (fingering 1) 'output-attributes '((id . "grob-calls")))
      ;; Interfaces and items (MS3 step 14): a grob's interfaces, its
      ;; class's in front of its definition's; an interface added; the item
      ;; calls. A grob's column, dead or alive, is a PaperColumn (MS3 step 15),
      ;; the one its X parents reach, with its when; a column's own is itself.
      (ly:grob-interfaces (fingering 0))
      (ly:grob-interfaces (string-number 0))
      (ly:grob-interfaces (stroke-finger 1))
      (ly:grob-interfaces (fingering 4))
      (ly:grob-interfaces 'x)
      (grob::has-interface (fingering 0) 'finger-interface)
      (grob::has-interface (fingering 0) 'item-interface)
      (grob::has-interface (fingering 0) 'string-number-interface)
      (hashq-ref (ly:all-grob-interfaces) 'finger-interface)
      (ly:add-interface 'grob-calls-interface "A probe's interface." '(grob-calls-padding))
      (hashq-ref (ly:all-grob-interfaces) 'grob-calls-interface)
      (grob::has-interface (fingering 0) 'grob-calls-interface)
      (ly:item? (fingering 0))
      (ly:item? (stroke-finger 0))
      (ly:item? 'x)
      (ly:item-break-dir (fingering 0))
      (ly:item-break-dir (stroke-finger 0))
      (ly:item-break-dir 'x)
      (ly:item-get-column (fingering 0))
      (ly:item-get-column (fingering 4))
      (ly:item-get-column (stroke-finger 0))
      (eq? (ly:item-get-column (fingering 0)) (ly:item-get-column (stroke-finger 0)))
      (eq? (ly:item-get-column (fingering 0)) (ly:item-get-column (fingering 1)))
      (eq? (ly:item-get-column (fingering 0)) (ly:item-get-column (ly:item-get-column (fingering 0))))
      (ly:grob-property (ly:item-get-column (fingering 1)) 'when)
      (ly:grob-property (ly:item-get-column (fingering 4)) 'when)
      (ly:item-break-dir (ly:item-get-column (fingering 0)))
      (ly:item-get-column 'x)
      ;; Grob arrays, pointer groups and units (MS3 step 14).
      (ly:grob-list->grob-array (list (fingering 0) (string-number 0) (fingering 0)))
      (ly:grob-array? (ly:grob-list->grob-array '()))
      (ly:grob-array? (list (fingering 0)))
      (ly:grob-array-length (ly:grob-list->grob-array (list (fingering 0) (fingering 1))))
      (ly:grob-array-ref (ly:grob-list->grob-array (list (fingering 0) (fingering 1))) 1)
      (ly:grob-array-ref (ly:grob-list->grob-array (list (fingering 0))) 1)
      (ly:grob-array->list (ly:grob-list->grob-array (list (stroke-finger 1) (fingering 0))))
      (ly:grob-array-filter (lambda (grob) (eq? (grob::name grob) 'Fingering))
                            (ly:grob-list->grob-array (list (fingering 0) (string-number 0) (fingering 1))))
      (ly:grob-list->grob-array (list (fingering 0) 'x))
      (ly:grob-list->grob-array (cons (fingering 0) (fingering 1)))
      (ly:grob-array-length '())
      (ly:grob-object (fingering 3) 'grob-calls-elements)
      (ly:pointer-group-interface::add-grob (fingering 3) 'grob-calls-elements (string-number 3))
      (ly:pointer-group-interface::add-grob (fingering 3) 'grob-calls-elements (stroke-finger 3))
      (ly:grob-object (fingering 3) 'grob-calls-elements)
      (ly:pointer-group-interface::add-grob (fingering 3) 'grob-calls-elements 'x)
      (ly:pt 1)
      (ly:bp 72)
      (ly:inch 1)
      (ly:cm 2.54)
      (ly:mm 10)
      (ly:mm 'x)
      ;; Pure and unpure calls on grobs (MS3 step 14): a container's pure
      ;; part, not kept; a container of no pure part, its unpure part kept;
      ;; pure heights and coordinates.
      (ly:grob-pure-property (fingering 1) 'minimum-X-extent 0 1)
      (ly:grob-pure-property (fingering 1) 'minimum-Y-extent 0 1)
      (ly:grob-pure-property (fingering 1) 'X-offset 0 1)
      (ly:grob-pure-property (fingering 1) 'no-such-property 0 1)
      (ly:grob-pure-property (fingering 1) 'no-such-property 0 1 'fallback)
      (ly:grob-set-property! (string-number 5) 'X-padding
                             (ly:make-unpure-pure-container (lambda (grob) 1) (lambda (grob start end) (+ start end 0.5))))
      (ly:grob-pure-property (string-number 5) 'X-padding 2 3)
      (ly:grob-property (string-number 5) 'X-padding)
      (ly:grob-pure-property (string-number 5) 'X-padding 2 3)
      (ly:grob-set-property! (string-number 5) 'X-padding (ly:make-unpure-pure-container (lambda (grob) 2)))
      (ly:grob-pure-property (string-number 5) 'X-padding 0 1)
      (ly:grob-property-data (string-number 5) 'X-padding)
      (ly:pure-call (ly:make-unpure-pure-container (lambda (grob) 1) (lambda (grob start end) (list start end))) (fingering 0) 0 2)
      (ly:unpure-call (ly:make-unpure-pure-container (lambda (grob) 1) (lambda (grob start end) (list start end))) (fingering 0))
      (ly:pure-call (lambda (grob) 1) (fingering 0) 0 2)
      (ly:pure-call 3 (fingering 0) 0 2)
      (ly:grob-pure-height (fingering 3) (fingering 3) 0 1)
      (ly:grob-pure-height (stroke-finger 1) (string-number 0) 0 1)
      (ly:grob-pure-height (fingering 3) (fingering 3) -1 1)
      (ly:grob-pure-relative-coordinate (stroke-finger 1) (fingering 0) 0 1)
      (ly:grob-pure-relative-coordinate (stroke-finger 1) (string-number 0) 0 1)
      ;; The rest of grob-scheme.cc's calls (MS3 step 14): a property's data,
      ;; nested properties, chained callbacks, the layout, the alist chain and
      ;; basic properties, the original, refpoints and extents. A grob's
      ;; system and rank go through its PaperColumn (MS3 step 15): no
      ;; column has a system before line breaking, and a column's rank is
      ;; its place in the System's columns. Its vertical order goes through
      ;; the VerticalAlignment, its axis group's place among the
      ;; alignment's elements (MS3 step 21), a dead grob's too, and -1 for
      ;; one with no alignment; two grobs of one axis group are not
      ;; compared. Its default font goes through its layout's parent, the
      ;; book's paper (MS3 step 14a).
      (ly:grob-property-data (fingering 0) 'padding)
      (ly:grob-property-data (fingering 0) 'direction)
      (ly:grob-property-data (fingering 2) 'padding)
      (ly:grob-property-data (fingering 3) 'positioning-done)
      (ly:grob-property-data (fingering 0) 'no-such-property)
      (ly:grob-property-data (fingering 0) "padding")
      (ly:grob-set-nested-property! (fingering 3) '(details grob-calls a) 1)
      (ly:grob-property (fingering 3) 'details)
      (ly:grob-set-nested-property! (fingering 3) '(details grob-calls b) 2)
      (ly:grob-set-nested-property! (fingering 3) '(details grob-calls a) 3)
      (ly:grob-property (fingering 3) 'details)
      (ly:grob-set-nested-property! (fingering 3) '(X-padding) 0.75)
      (ly:grob-property (fingering 3) 'X-padding)
      (ly:grob-set-nested-property! (fingering 3) '(details "a") 1)
      (ly:grob-set-property! (fingering 3) 'X-padding (lambda (grob) 2))
      (ly:grob-chain-callback (fingering 3) (lambda (grob value) (* 10 value)) 'X-padding)
      (ly:grob-property (fingering 3) 'X-padding)
      (ly:grob-chain-callback (fingering 3) (lambda (grob value) (list 'chained value)) 'details)
      (ly:grob-property (fingering 3) 'details)
      (ly:grob-chain-callback (fingering 3) 3 'X-padding)
      (ly:output-def? (ly:grob-layout (fingering 0)))
      (eq? (ly:grob-layout (fingering 0)) (ly:grob-layout (stroke-finger 1)))
      (length (ly:grob-alist-chain (fingering 0)))
      (eq? (cadr (ly:grob-alist-chain (fingering 0))) (ly:grob-basic-properties (fingering 0)))
      (assq-ref (car (ly:grob-alist-chain (fingering 0))) 'padding)
      (cddr (ly:grob-alist-chain (fingering 0) '(((padding . 9)))))
      (assq-ref (ly:grob-basic-properties (fingering 0)) 'font-size)
      (ly:grob-basic-properties (fingering 4))
      (ly:grob-original (fingering 0))
      (ly:grob-original (fingering 4))
      (ly:grob-robust-relative-extent (string-number 0) (fingering 0) X)
      (ly:grob-robust-relative-extent (stroke-finger 1) (string-number 0) Y)
      (ly:grob-common-refpoint-of-array (stroke-finger 1) (ly:grob-list->grob-array (list (string-number 0) (fingering 0))) X)
      (ly:grob-common-refpoint-of-array (stroke-finger 1) (ly:grob-list->grob-array '()) Y)
      (ly:grob-common-refpoint-of-array (stroke-finger 1) (list (fingering 0)) X)
      (ly:grob-system (fingering 0))
      (ly:grob-system (fingering 4))
      (ly:grob-system (ly:item-get-column (fingering 0)))
      (ly:grob-system 'x)
      (ly:grob-vertical<? (fingering 0) 'x)
      (ly:grob-get-vertical-axis-group-index (fingering 0))
      (ly:grob-get-vertical-axis-group-index (fingering 3))
      (ly:grob-get-vertical-axis-group-index (fingering 4))
      (ly:grob-get-vertical-axis-group-index (string-number 3))
      (ly:grob-get-vertical-axis-group-index (stroke-finger 0))
      (ly:grob-get-vertical-axis-group-index (stroke-finger 1))
      (ly:grob-get-vertical-axis-group-index (stroke-finger 3))
      (ly:grob-get-vertical-axis-group-index 'x)
      (ly:grob-vertical<? (fingering 0) (fingering 3))
      (ly:grob-vertical<? (fingering 3) (fingering 0))
      (ly:grob-vertical<? (string-number 3) (fingering 4))
      (ly:grob-vertical<? (fingering 4) (string-number 3))
      (ly:grob-vertical<? (stroke-finger 1) (fingering 3))
      (ly:grob-vertical<? 'x (fingering 0))
      (ly:grob-spanned-column-rank-interval (fingering 0))
      (ly:grob-spanned-column-rank-interval (fingering 4))
      (ly:grob-spanned-column-rank-interval (stroke-finger 3))
      (ly:grob-spanned-column-rank-interval (ly:item-get-column (fingering 1)))
      (ly:grob-spanned-column-rank-interval 'x)
      (ly:font-name (ly:grob-default-font (fingering 3)))
      (ly:font-magnification (ly:grob-default-font (fingering 3)))
      (eq? (ly:grob-default-font (fingering 3)) (ly:grob-property (fingering 3) 'font))
      (ly:grob-default-font 'x)))
  ;; A stencil for each grob that no call sets, a box of its own size, so
  ;; that no extent and no page needs a font.
  (define (grob-calls-stencil! grob n)
    (ly:grob-set-property! grob 'stencil
                           (ly:make-stencil (list 'round-filled-box 0 (* 0.1 n) 0 1 0)
                                            (cons 0 (* 0.1 n)) '(0 . 1))))
  (define (grob-calls-recorder context)
    (make-engraver
     (acknowledgers
      ((grob-interface engraver grob source-engraver)
       (when (memq (grob::name grob) '(Fingering StringNumber StrokeFinger))
         (set! grob-calls-heard (cons (cons grob (grob::name grob)) grob-calls-heard)))))
     ((finalize engraver)
      (set! grob-calls-heard (reverse grob-calls-heard))
      (let ((n 0))
        (for-each (lambda (h) (set! n (1+ n)) (grob-calls-stencil! (car h) n)) grob-calls-heard))
      (let ((port (open-file (or (getenv "GROB_CALLS_OUT") "grob-calls.txt") "w")))
        (set-port-encoding! port "UTF-8")
        (for-each (lambda (call) (grob-calls-run port call)) grob-calls)
        (close-port port))))))

\layout {
  \context { \Score \consists #grob-calls-recorder }
}

\include "fixtures/grobs.ly"
