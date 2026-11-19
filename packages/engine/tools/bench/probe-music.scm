;;; probe-music.scm: lengths, starts, equal? and music-functions.scm's walkers on frozen music (N2, MS2 step 3)
;; Run in (lily) once the library files and music-functions-init.ly are in,
;; by the engine from source or from an image, and by LilyPond 2.26.0 through
;; `#(primitive-load "probe-music.scm")`; one line per probe, which must be
;; the same everywhere. tools/bench/test/image.test.mjs runs it.
;;
;; Up to the line `;;; probes`, the file makes the music: built live with
;; make-music, then each frozen in place by %music-freeze!, which LilyPond
;; does not have and does without. The image test writes an image with this
;; part run before it, and runs the probes alone in VMs restored from it, so
;; that they read frozen music the image saved.
(set-current-module (resolve-module '(lily)))
(define freeze (if (defined? '%music-freeze!) %music-freeze! identity))

(define (note step log . dots)
  (make-music 'NoteEvent
              'pitch (ly:make-pitch 0 step 0)
              'duration (ly:make-duration log (if (pair? dots) (car dots) 0))))
(define (rest log) (make-music 'RestEvent 'duration (ly:make-duration log)))
(define (seq . elements) (make-music 'SequentialMusic 'elements elements))
(define (sim . elements) (make-music 'SimultaneousMusic 'elements elements))
(define (chord . elements) (make-music 'EventChord 'elements elements))
(define (wrap type element . props) (apply make-music type 'element element props))

;; Each music of the probes by name, and a copy frozen for each.
(define probe-music
  (list
   (cons 'note (note 0 2))
   (cons 'dotted (note 1 3 2))
   (cons 'no-duration (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0)))
   (cons 'with-length (make-music 'NoteEvent 'duration (ly:make-duration 2) 'length (ly:make-moment 5)))
   (cons 'seq (seq (note 0 2) (rest 3) (note 2 3 1)))
   (cons 'empty-seq (seq))
   (cons 'sim (sim (note 0 1) (seq (note 0 2) (note 1 2) (note 2 2))))
   (cons 'chord (chord (note 0 2) (note 2 2) (note 4 1)))
   (cons 'chord-duration (make-music 'EventChord 'elements (list (note 0 2)) 'duration (ly:make-duration 1)))
   (cons 'grace (seq (wrap 'GraceMusic (seq (note 0 3) (note 1 4))) (note 2 2)))
   (cons 'graces (seq (wrap 'GraceMusic (note 0 4)) (wrap 'GraceMusic (note 1 3)) (note 2 2) (wrap 'GraceMusic (note 3 4))))
   (cons 'sim-grace (sim (seq (wrap 'GraceMusic (note 0 4)) (note 1 2)) (note 2 1)))
   (cons 'tuplet (wrap 'TimeScaledMusic
                       (seq (note 0 3 0) (note 1 3) (note 2 3))
                       'numerator 2 'denominator 3))
   (cons 'relative (wrap 'RelativeOctaveMusic (seq (note 0 2) (note 4 2))))
   (cons 'context (wrap 'ContextSpeccedMusic (seq (note 0 1) (rest 1)) 'context-type 'Voice))
   (cons 'infinite (sim (make-music 'LyricCombineMusic) (make-music 'LyricCombineMusic)))
   (cons 'mixed-infinite (sim (make-music 'LyricCombineMusic) (note 0 1)))
   (cons 'mm-rest (make-music 'MultiMeasureRestMusic 'duration (ly:make-duration 0 0 3)))
   (cons 'unfold (wrap 'UnfoldedRepeatedMusic (seq (note 0 2) (note 1 2)) 'repeat-count 3))
   (cons 'volta (wrap 'VoltaRepeatedMusic (seq (note 0 2)) 'repeat-count 2))
   (cons 'tremolo (wrap 'TremoloRepeatedMusic (seq (note 0 3) (note 2 3)) 'repeat-count 4))
   (cons 'repeat-inside (seq (note 0 2) (wrap 'UnfoldedRepeatedMusic (seq (note 1 2)) 'repeat-count 2) (note 2 1)))
   (cons 'deep (seq (sim (seq (wrap 'GraceMusic (note 0 4)) (chord (note 0 2) (note 2 2)))
                         (wrap 'ContextSpeccedMusic (seq (note 4 1)) 'context-type 'Voice))
                    (wrap 'TimeScaledMusic (seq (note 0 3) (note 1 3) (note 2 3)) 'numerator 2 'denominator 3)))))

(define probe-frozen
  (map (lambda (entry) (cons (car entry) (freeze (ly:music-deep-copy (cdr entry)))))
       probe-music))

;;; probes
(set-current-module (resolve-module '(lily)))
(define (probe label value)
  (write label)
  (display " ")
  (write value)
  (newline))
(define (music-names m)
  (let ((name (ly:music-property m 'name))
        (elements (ly:music-property m 'elements))
        (element (ly:music-property m 'element)))
    (cons name
          (append (if (ly:music? element) (list (music-names element)) '())
                  (map music-names (filter ly:music? elements))))))

;; Lengths and starts, live and frozen.
(for-each
 (lambda (live frozen)
   (probe (car live)
          (list (ly:music-length (cdr live)) (ly:music-start (cdr live))
                (ly:music-length (cdr frozen)) (ly:music-start (cdr frozen)))))
 probe-music probe-frozen)

;; The callbacks called as Scheme calls them.
(let ((seq (assq-ref probe-frozen 'seq))
      (sim (assq-ref probe-frozen 'sim))
      (grace (assq-ref probe-frozen 'grace))
      (tuplet (assq-ref probe-frozen 'tuplet))
      (repeat (assq-ref probe-frozen 'repeat-inside)))
  (probe 'callbacks
         (list (ly:music-sequence::cumulative-length-callback seq)
               (ly:music-sequence::maximum-length-callback seq)
               (ly:music-sequence::minimum-start-callback sim)
               (ly:music-sequence::first-start-callback grace)
               (ly:music-sequence::cumulative-length-callback repeat)
               (ly:music-wrapper::length-callback tuplet)
               (ly:music-wrapper::start-callback tuplet)
               (ly:music::duration-length-callback (assq-ref probe-frozen 'dotted))
               (ly:music::duration-length-callback seq))))
(probe 'errors
       (map (lambda (f) (catch #t (lambda () (f 3)) (lambda (key subr . rest) (list key subr))))
            (list ly:music-length ly:music-start ly:music-sequence::cumulative-length-callback
                  ly:music-wrapper::length-callback ly:grace-music::start-callback
                  ly:calculated-sequential-music::length)))

;; equal?: origins left out, live and frozen alike.
(let* ((live (assq-ref probe-music 'deep))
       (frozen (assq-ref probe-frozen 'deep))
       (moved (ly:music-deep-copy live))
       (changed (ly:music-deep-copy frozen)))
  (ly:music-set-property! moved 'origin (*location*))
  (ly:music-set-property! (car (ly:music-property changed 'elements)) 'duration (ly:make-duration 1))
  (probe 'equal
         (list (equal? live frozen) (equal? frozen live) (equal? frozen (ly:music-deep-copy frozen))
               (equal? moved frozen) (equal? changed frozen) (equal? changed live)
               (equal? (assq-ref probe-frozen 'seq) (assq-ref probe-frozen 'sim))
               (equal? (note 0 2) (make-music 'NoteEvent 'duration (ly:make-duration 2) 'pitch (ly:make-pitch 0 0 0)))
               (equal? (note 0 2) (make-music 'RestEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration 2)))
               (equal? (list frozen 1) (list live 1)))))

;; music-functions.scm's walkers, on copies so that the frozen music stays
;; frozen: the image test checks that nothing above thawed it.
(let ((deep (assq-ref probe-frozen 'deep)))
  (probe 'map
         (let ((m (music-map (lambda (m)
                               (if (ly:duration? (ly:music-property m 'duration))
                                   (ly:music-set-property! m 'duration (ly:make-duration 3)))
                               m)
                             (ly:music-deep-copy deep))))
           (list (ly:music-length m) (ly:music-start m) (ly:music-length deep))))
  (probe 'filter
         (music-names (music-filter (lambda (m) (not (music-is-of-type? m 'grace-music)))
                                    (ly:music-deep-copy deep))))
  (probe 'some
         (let ((pitches '()))
           (for-some-music (lambda (m)
                             (let ((p (ly:music-property m 'pitch)))
                               (and (ly:pitch? p)
                                    (begin (set! pitches (cons p pitches)) #t))))
                           (ly:music-deep-copy deep))
           (reverse pitches)))
  (probe 'names (music-names (ly:music-deep-copy deep)))
  (probe 'unchanged (list (equal? deep (assq-ref probe-music 'deep)) (ly:music-length deep))))
