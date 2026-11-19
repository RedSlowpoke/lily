;;; probe-library.scm: run in (lily) once lily-library.scm and music-functions.scm have loaded, from source or from an image
;; One line per probe: the value as write writes it, or the error's key and
;; arguments; never a value whose printing shows where it is in the heap, as
;; a procedure without a name does. The first 31 are tests/library.rs' PROBES, which it checks
;; against LilyPond 2.26.0. tests/image.rs runs the file in a VM that loaded
;; the two files from source and in one restored from its image, and the two
;; outputs must be the same.
(set-current-module (resolve-module '(lily)))
(define (probe form)
  (catch #t
    (lambda () (write (primitive-eval form)))
    (lambda (key . args) (display key) (display " ") (write args)))
  (newline))

;; tests/library.rs' PROBES, in its order.
(probe '(list X Y LEFT RIGHT UP DOWN CENTER SHARP DOUBLE-FLAT INFINITY-INT))
(probe '(list PI TWO-PI PI-OVER-TWO PI-OVER-180))
(probe '(map-selected-alist-keys - '(a b) '((a . 1) (b . -2) (c . 3) (d . 4))))
(probe '(split-list-by-separator '(a 0 b c 1 d) number?))
(probe '(split-list-by-group-lengths '(a b c d e f) '(3 2 1)))
(probe '(flat-zip-longest '(a b c d) '(e f) '(g h i)))
(probe '(group-into-ranges '(1 4 3 6 7 2)))
(probe '(uniqued-alist '((a . 1) (b . 2) (a . 3))))
(probe '(list-insert-separator '(a b c) 'x))
(probe '(list INF-MOMENT ZERO-MOMENT ZERO-DURATION (moment-min (ly:make-moment 1/4) (ly:make-moment 1/4 -1/8)) (moment<=? ZERO-MOMENT ZERO-MOMENT)))
(probe '(list (fraction->moment '(3 . 4)) (fraction->moment '()) (moment->fraction (ly:make-moment 6/8 -1/8)) (moment->fraction INF-MOMENT)))
(probe '(list (duration-visual (ly:make-duration 3 1 2/3)) (duration-visual-length (ly:make-duration 3 1 2/3)) (make-duration-of-length (ly:make-moment 3/8)) (make-duration-of-length (ly:make-moment 5/16))))
(probe '(list (duration-log-factor 3) (duration-log-factor -1) (duration-dot-factor 2)))
(probe '(duration-log-factor 1.5))
(probe '(list (string-endswith "hello" "LO" #:case-insensitive #t) (string-startswith "hello" "he") (string-encode-integer 1234) (remove-whitespace " a b ")))
(probe '(list (interval-union '(0 . 1) '(2 . 3)) (interval-length '(1 . 5)) (ordered-cons 3 1) (interval-widen '(1 . 2) 1)))
(probe '(list (binary-search 0 10 (lambda (x) (* x x)) 50) (bit-list->int '(#t #f #t)) (byte-list->bit-list '(5))))
(probe '(list (car< '(1 . a) '(2 . b)) (symbol<? 'b 'a) (symbol-key<? '(a . 1) '(b . 2))))
(probe '(list (magstep 6) (magnification->font-size 2)))
(probe '(list (ly:version? > '(2 20 0)) (lilypond-version-outdated? '(2 24 0) '(2 26 0))))
(probe '(@@ (lily) convert-ly-msg))
(probe '(list (ly:regex-match? 5) (ly:regex-match? (ly:make-regex "a"))))
(probe '(call-with-output-bytevector (lambda (p) (display "ab" p))))
(probe '(eval-carefully '(+ 1 2) (resolve-module '(lily))))
(probe '(list (get-next-unique-voice-name) (get-next-unique-voice-name)))
(probe '(length toplevel-music-functions))
(probe '(map car accidental-styles))
(probe 'score-grace-settings)
(probe '(list (define-tag-group '(engine-probe-a engine-probe-b)) (define-tag-group '(engine-probe-a)) (define-tag-group 5)))
(probe '(let ((p (lambda (x) x))) (set! (syntax-function-procedure-arguments p) '(x)) (syntax-function-procedure-arguments p)))
(probe '(list (eq? 'a 'a 'a) (eqv? 1 1 2) (equal?)))

;; The image's own: what loading left in the heap and the modules.
(probe '(list (get-next-unique-voice-name) (define-tag-group '(engine-probe-b))))
(probe '(list <regex-match> (class-name <regex-match>) (is-a? 5 <regex-match>)))
(probe '(assert (= 1 1)))
(probe '(assert (eq? 'a 'b)))
(probe '(let ((e (make-engraver ((initialize engraver) (list 'init engraver))
                                (acknowledgers ((note-head-interface engraver grob source) grob)))))
          (list (map car e) ((assq-ref e 'initialize) 'me)
                ((assq-ref (assq-ref e 'acknowledgers) 'note-head-interface) 1 2 3))))
(probe '(begin (set! unique-counter (+ unique-counter 10)) (get-next-unique-voice-name)))
(probe '(map (lambda (m) (module-name (resolve-module m))) '((lily) (srfi srfi-1) (ice-9 match) (lily display-lily))))
(probe '(list (fold + 0 '(1 2 3)) (delete-duplicates '(a b a c b)) (match '(1 (2 3)) ((a (b c)) (list c b a)))))
(probe '(list (ly:make-pitch 1 2 SHARP) (ly:pitch-transpose (ly:make-pitch 0 4 FLAT) (ly:make-pitch -1 3 0)) (ly:default-scale)))
(probe '(list (expt 2 100) (/ (expt 3 50) (expt 2 70)) (exact->inexact 1/3) (make-rectangular 1.5 -2)))
(probe '(let ((h (make-weak-key-hash-table))) (hashq-set! h 'k 1) (list (hashq-ref h 'k) (hash-table->alist h))))
(probe '(list (procedure? ly:assoc-get) ly:make-moment (procedure-name ly:make-duration)))
