;;; probe-grobs.scm: calls on grobs (01 §3.5's L1 parts 1 and 2, MS3 steps 13 to 15 and 21)
;; tools/compare/grob-calls.ly's 297 calls, as LilyPond 2.26.0 ran them on
;; fixtures/grobs.ly's grobs ($LILY_CORPUS/ref/grob-calls/grob-calls.txt),
;; in its order and its sections, run by the engine on its own grobs:
;;
;; - in a compile of the fixture by the core, from its image natively
;;   (examples/compile.rs) and in wasm (core.wasm), whose Score hears the
;;   fixture's grobs as grob-calls.ly's recorder hears them and runs the
;;   probe in its finalize (tools/compare/test/grobs.test.mjs): every
;;   section;
;; - natively in a VM with the music crate, layout and output-lib.scm's
;;   callbacks, on hand-made grobs that stand for the fixture's first nine
;;   (crates/layout/tests/grobs.rs): the chains and the parents, offsets and
;;   extents, the sections such grobs answer as the fixture's do.
;;
;; The sections: properties with no callback, set on the grob and from its
;; definition, ly:grob-property's default and the cause; callbacks called
;; once and kept, unpure-pure containers, a callback that throws; the
;; chains, output-lib.scm's grob::compose-function and grob::offset-function
;; over plain data, a procedure, a container, a definition's data and each
;; other, as grob-closure.cc chains them; cycles; parents, offsets and
;; extents on both axes (relative coordinates, extents with minimum extents
;; and an explicit X-extent, common refpoints, translate-axis before and
;; after an offset is computed, ly:grob::x-parent-positioning); the grob's
;; warnings at its origin; each grob's link; suicide; the properties the
;; page's print stencils read; and MS3 step 14's calls after them:
;; interfaces and items, grob arrays, pointer groups and units, pure and
;; unpure calls on grobs, and the rest of grob-scheme.cc's. Those read the
;; fixture's definitions and the earlier sections' grobs (a dead one, a
;; kept container), so they run in the compiles only.
;;
;; The host binds, in (lily), grob-calls-heard, the grobs with their types
;; ((grob . Fingering) and so on) in the order grob-calls.ly's recorder hears
;; them, and may bind probe-grobs-sections, the names of the sections to
;; run (all of them else). As grob-calls.ly does, the probe names the nth
;; grob of each type (fingering n), (string-number n) and (stroke-finger n),
;; and gives each grob heard a stencil of its own size before the calls. A
;; link is output-svg.scm's grob-cause in grob-calls.ly; the host that
;; compiles the fixture binds grob-calls-link to the engine's writer's
;; (crates/svg's grob-cause, read through layout's %grob-cause-link) in
;; output-svg.scm's <a> (MS3 step 13a). A call of grob-calls-link runs only
;; where the host binds it. For each call run a record, as grob-calls.ly
;; writes it: the call as `write` writes it, a tab, its value as
;; probe-grobs-show writes it (grob-calls-show's; a grob array as
;; (grob-array <grob> …)), and a newline. A record
;; is the capture's record of the same call.
(set-current-module (resolve-module '(lily)))
;; grob-calls.ly's counters of callbacks' calls.
(define grob-calls-count 0)
(define grob-calls-positioning 0)
(define (grob-calls-nth type n)
  (car (list-ref (filter (lambda (h) (eq? (cdr h) type)) grob-calls-heard) n)))
(define (fingering n) (grob-calls-nth 'Fingering n))
(define (string-number n) (grob-calls-nth 'StringNumber n))
(define (stroke-finger n) (grob-calls-nth 'StrokeFinger n))
(define (grob-calls-definition type property)
  (assq-ref (assq-ref all-grob-descriptions type) property))
(define (probe-grobs-show x)
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
        ((ly:grob-array? x) (cons 'grob-array (map probe-grobs-show (ly:grob-array->list x))))
        ((ly:unpure-pure-container? x) 'unpure-pure-container)
        ((procedure? x) 'procedure)
        ((unspecified? x) 'unspecified)
        ((pair? x) (cons (probe-grobs-show (car x)) (probe-grobs-show (cdr x))))
        ((vector? x) (list->vector (map probe-grobs-show (vector->list x))))
        (else x)))
(define (probe-grobs-run call)
  (write call)
  (display "\t")
  (write (catch #t
           (lambda () (probe-grobs-show (primitive-eval call)))
           (lambda (key . args) (list 'error key))))
  (newline))
(let ((n 0))
  (for-each (lambda (h)
              (set! n (1+ n))
              (ly:grob-set-property! (car h) 'stencil
                                     (ly:make-stencil (list 'round-filled-box 0 (* 0.1 n) 0 1 0)
                                                      (cons 0 (* 0.1 n)) '(0 . 1))))
            grob-calls-heard))
(define probe-grobs-calls '(
(properties
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
(grob::name (stroke-finger 1)))
(callbacks
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
(ly:grob-property (fingering 1) 'slur-padding))
(chains
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
(ly:grob-property (string-number 2) 'padding))
(cycles
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
(ly:grob-set-property! (string-number 3) 'Y-offset 0))
(parents
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
(ly:grob-extent (string-number 1) (string-number 1) X))
(warnings
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
(ly:grob-relative-coordinate (fingering 5) (fingering 5) X))
(links
;; Print stencils: each grob's link, as the page's writer makes it.
(grob-calls-link (fingering 0))
(grob-calls-link (fingering 1))
(grob-calls-link (fingering 2))
(grob-calls-link (fingering 3))
(grob-calls-link (fingering 4))
(grob-calls-link (fingering 5))
(grob-calls-link (string-number 0))
(grob-calls-link (stroke-finger 0))
(grob-calls-link (stroke-finger 1)))
(suicide
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
(ly:grob-property (stroke-finger 0) 'cause))
(page
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
(ly:grob-set-property! (fingering 1) 'output-attributes '((id . "grob-calls"))))
(items
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
(ly:item-get-column 'x))
(arrays
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
(ly:mm 'x))
(pure
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
(ly:grob-pure-relative-coordinate (stroke-finger 1) (string-number 0) 0 1))
(face
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
(ly:grob-default-font 'x))))
(define probe-grobs-run-sections
  (if (defined? 'probe-grobs-sections) probe-grobs-sections (map car probe-grobs-calls)))
(for-each (lambda (section)
            (when (memq (car section) probe-grobs-run-sections)
              (for-each (lambda (call)
                          (unless (and (pair? call) (eq? (car call) 'grob-calls-link)
                                       (not (defined? 'grob-calls-link)))
                            (probe-grobs-run call)))
                        (cdr section))))
          probe-grobs-calls)
