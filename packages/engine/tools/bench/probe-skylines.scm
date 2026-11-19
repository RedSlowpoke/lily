;;; probe-skylines.scm: skyline calls on built skylines (MS3 step 26)
;; Calls of skyline-scheme.cc's ly:skyline-* functions on skylines that
;; ly:make-skyline builds from segments, run in (lily): by LilyPond 2.26.0
;; (tools/compare/skyline-calls.ly), and by crates/skyline's port
;; (tests/skyline.rs reads the skylines and the calls from this file and
;; makes the same ones); tools/bench/test/skylines.test.mjs compares the
;; two to four decimals. The calls reach Skyline (segments), merge, padded,
;; distance and both internal_distance, height, max_height, is_empty and
;; to_points; raise, shift, right, set_minimum_height, clear, the boxes'
;; constructors and Skyline_pair have no ly: function and are not here.
;; No call asks what LilyPond asserts against (a height at an infinity, the
;; distance of skylines of one side, merging skylines of two sides).
;;
;; tests/skyline.rs knows only what is written here: `(define probe-… expr)`
;; of a skyline, and the calls of the list given to probe-run, made of
;; ly:make-skyline, the ly:skyline-* functions, `list`, quoted segment lists,
;; numbers (also n/d and the infinities) and X, Y, UP and DOWN.
;;
;; For each call one line: the numbers and booleans of its value, flattened,
;; as `write` writes them and separated by spaces; or `error`, its key and
;; its message, which LilyPond gives for no call here.
(set-current-module (resolve-module '(lily)))
(define (probe-flat x)
  (cond ((pair? x) (append (probe-flat (car x)) (probe-flat (cdr x))))
        ((null? x) '())
        (else (list x))))
(define (probe-run call)
  (catch #t
    (lambda ()
      (let loop ((values (probe-flat (primitive-eval call))) (first #t))
        (if (pair? values)
            (begin (if (not first) (display " "))
                   (write (car values))
                   (loop (cdr values) #f)))))
    (lambda (key . args)
      (display "error ")
      (display key)
      (display " ")
      (write args)))
  (newline))

;; Horizon X: above, overlapping and sloped; below; segments right to left,
;; of no length, vertical and repeated.
(define probe-up (ly:make-skyline '((0 0 1 1) (1 1 3 1) (2 0.5 4 2) (3.5 -1 5 -1)) X UP))
(define probe-down (ly:make-skyline '((0.5 -1 2.5 -0.2) (-1 -0.5 1 -0.5) (3 0 6 -2)) X DOWN))
(define probe-odd (ly:make-skyline '((2 1 0 0) (1 3 1 4) (1 2 1 2) (0 0.5 2 0.5) (0 0.5 2 0.5) (-1 0 -1/3 1/3)) X UP))
;; Horizon Y: segments as (x y x y), y along the horizon.
(define probe-left (ly:make-skyline '((0 0 0.5 1) (1 -1 1 2) (-0.25 1.5 0.75 3)) Y DOWN))
(define probe-right (ly:make-skyline '((2 0 1.5 1) (2.5 -0.5 2.5 0.5) (1/3 -2 1/7 2.75)) Y UP))
;; Empty, and flat to the infinities.
(define probe-empty-up (ly:make-skyline '() X UP))
(define probe-empty-down (ly:make-skyline '() X DOWN))
(define probe-floor (ly:make-skyline '((-inf.0 -2 +inf.0 -2)) X UP))
(define probe-ceiling (ly:make-skyline '((-inf.0 3 0 3) (0 1.5 +inf.0 1.5)) X DOWN))
;; A 16-gon of radius 1.5 about (1, 0.5), both sides of it.
(define probe-gon-up (ly:make-skyline '((2.5 0.5 2.3858 1.074) (2.3858 1.074 2.0607 1.5607) (2.0607 1.5607 1.574 1.8858) (1.574 1.8858 1 2) (1 2 0.426 1.8858) (0.426 1.8858 -0.0607 1.5607) (-0.0607 1.5607 -0.3858 1.074) (-0.3858 1.074 -0.5 0.5) (-0.5 0.5 -0.3858 -0.074) (-0.3858 -0.074 -0.0607 -0.5607) (-0.0607 -0.5607 0.426 -0.8858) (0.426 -0.8858 1 -1) (1 -1 1.574 -0.8858) (1.574 -0.8858 2.0607 -0.5607) (2.0607 -0.5607 2.3858 -0.074) (2.3858 -0.074 2.5 0.5)) X UP))
(define probe-gon-down (ly:make-skyline '((2.5 0.5 2.3858 1.074) (2.3858 1.074 2.0607 1.5607) (2.0607 1.5607 1.574 1.8858) (1.574 1.8858 1 2) (1 2 0.426 1.8858) (0.426 1.8858 -0.0607 1.5607) (-0.0607 1.5607 -0.3858 1.074) (-0.3858 1.074 -0.5 0.5) (-0.5 0.5 -0.3858 -0.074) (-0.3858 -0.074 -0.0607 -0.5607) (-0.0607 -0.5607 0.426 -0.8858) (0.426 -0.8858 1 -1) (1 -1 1.574 -0.8858) (1.574 -0.8858 2.0607 -0.5607) (2.0607 -0.5607 2.3858 -0.074) (2.3858 -0.074 2.5 0.5)) X DOWN))
;; 40 segments, more than libc++'s sort puts in order by insertion.
(define probe-many (ly:make-skyline '(
  (1.94 0.17 2.84 1.95) (3.44 1.48 5.03 1.66) (-8.51 -1.16 -5.52 -1.23) (9.86 -0.46 10.03 -1.88)
  (-8.56 1.7 -6.54 1.24) (4.82 0.47 5.02 -0.02) (-5.92 -1.72 -3.25 1.21) (-7.07 0.87 -6.99 0.96)
  (7.26 1.34 9.72 0.97) (-9.28 -1.4 -8.26 0.64) (-2.34 1.16 -1.44 -0.75) (7.49 -1.2 9.16 -0.15)
  (1.48 -0.81 2.73 1.36) (2.02 0.87 3.08 0.35) (-2.01 -1.05 -0.29 -1.66) (1.83 -0.18 3.4 -0.82)
  (-0.96 1.08 -0.72 -1.19) (-7.59 0.42 -5.04 0.47) (8.64 -0.08 9.39 -0.43) (2.63 -1.07 4.67 -1.23)
  (2.35 -0.87 2.35 1.11) (5.33 -0.53 6.62 0.04) (-4.92 1.37 -4.04 -0.87) (1.89 1.5 2.07 0.75)
  (-1.03 1.31 -0.51 -0.85) (-5.57 1.1 -4.21 -0.54) (-9.98 -1.57 -8.72 1.96) (-4.45 0.37 -2.72 1.59)
  (4.42 -0.97 7.35 -1.09) (-0.09 1.14 1.09 0.03) (8.88 0.02 11.22 1.66) (-2.21 1.55 -0.46 -0.15)
  (-3.25 1.59 -1.01 1.5) (-6.39 -0.64 -5.65 -1.8) (-7.28 1.26 -5.29 -1.06) (1.05 -1.26 1.91 0.4)
  (2.86 1.65 5.61 -1.02) (3.62 1.29 3.98 -0.66) (-3.76 -1.56 -1.68 1.38) (-2.9 0.45 -2.07 -0.11)) X DOWN))

(for-each probe-run '(
;; the points, along either horizon axis
(ly:skyline->points probe-up X)
(ly:skyline->points probe-up Y)
(ly:skyline->points probe-down X)
(ly:skyline->points probe-odd X)
(ly:skyline->points probe-left Y)
(ly:skyline->points probe-right Y)
(ly:skyline->points probe-right X)
(ly:skyline->points probe-empty-up X)
(ly:skyline->points probe-empty-down X)
(ly:skyline->points probe-floor X)
(ly:skyline->points probe-ceiling X)
(ly:skyline->points probe-gon-up X)
(ly:skyline->points probe-gon-down X)
(ly:skyline->points probe-many X)
;; emptiness
(list (ly:skyline-empty? probe-up) (ly:skyline-empty? probe-empty-up) (ly:skyline-empty? probe-empty-down) (ly:skyline-empty? probe-floor) (ly:skyline-empty? (ly:make-skyline '((1 1 1 2)) X UP)))
;; merging
(ly:skyline->points (ly:skyline-merge probe-up probe-odd) X)
(ly:skyline->points (ly:skyline-merge probe-odd probe-up) X)
(ly:skyline->points (ly:skyline-merge probe-up probe-empty-up) X)
(ly:skyline->points (ly:skyline-merge probe-empty-up probe-up) X)
(ly:skyline->points (ly:skyline-merge probe-up probe-floor) X)
(ly:skyline->points (ly:skyline-merge probe-down probe-ceiling) X)
(ly:skyline->points (ly:skyline-merge probe-gon-down probe-many) X)
(ly:skyline->points (ly:skyline-merge probe-up (ly:make-skyline '((0 0 1 1) (1 1 3 1)) X UP)) X)
;; heights, at the buildings' ends and between them
(list (ly:skyline-height probe-up -1) (ly:skyline-height probe-up 0) (ly:skyline-height probe-up 0.5) (ly:skyline-height probe-up 1) (ly:skyline-height probe-up 2.5) (ly:skyline-height probe-up 3.75) (ly:skyline-height probe-up 1e6))
(list (ly:skyline-height probe-down 0) (ly:skyline-height probe-down 1) (ly:skyline-height probe-down 2.5) (ly:skyline-height probe-down 4.5) (ly:skyline-height probe-down -2))
(list (ly:skyline-height probe-left 0.5) (ly:skyline-height probe-left 2) (ly:skyline-height probe-right 1/3) (ly:skyline-height probe-right 0))
(list (ly:skyline-height probe-empty-up 0) (ly:skyline-height probe-empty-down 0) (ly:skyline-height probe-floor 7) (ly:skyline-height probe-ceiling 0) (ly:skyline-height probe-ceiling -0.5))
(list (ly:skyline-height probe-gon-up 1) (ly:skyline-height probe-gon-up 0.1) (ly:skyline-height probe-gon-down 2.2) (ly:skyline-height probe-many -3.3) (ly:skyline-height probe-many 2.35))
;; the maxima
(list (ly:skyline-max-height probe-up) (ly:skyline-max-height probe-down) (ly:skyline-max-height probe-odd) (ly:skyline-max-height probe-left) (ly:skyline-max-height probe-right))
(list (ly:skyline-max-height probe-empty-up) (ly:skyline-max-height probe-empty-down) (ly:skyline-max-height probe-floor) (ly:skyline-max-height probe-ceiling) (ly:skyline-max-height probe-gon-up) (ly:skyline-max-height probe-gon-down) (ly:skyline-max-height probe-many))
;; padding
(ly:skyline->points (ly:skyline-pad probe-up 0.25) X)
(ly:skyline->points (ly:skyline-pad probe-up 1/3) X)
(ly:skyline->points (ly:skyline-pad probe-down 0.5) X)
(ly:skyline->points (ly:skyline-pad probe-odd 0.1) X)
(ly:skyline->points (ly:skyline-pad probe-left 0.2) Y)
(ly:skyline->points (ly:skyline-pad probe-right 2) Y)
(ly:skyline->points (ly:skyline-pad probe-up 0) X)
(ly:skyline->points (ly:skyline-pad probe-up -0.5) X)
(ly:skyline->points (ly:skyline-pad probe-empty-up 0.5) X)
(ly:skyline->points (ly:skyline-pad probe-floor 0.5) X)
(ly:skyline->points (ly:skyline-pad probe-ceiling 0.5) X)
(ly:skyline->points (ly:skyline-pad probe-gon-up 0.3) X)
(ly:skyline->points (ly:skyline-pad probe-many 0.15) X)
(ly:skyline->points (ly:skyline-pad (ly:skyline-pad probe-up 0.25) 0.25) X)
;; distances, without and with horizon padding, either way round
(list (ly:skyline-distance probe-up probe-down) (ly:skyline-distance probe-down probe-up))
(list (ly:skyline-distance probe-up probe-down 0) (ly:skyline-distance probe-up probe-down 0.1) (ly:skyline-distance probe-up probe-down 0.5) (ly:skyline-distance probe-down probe-up 0.5) (ly:skyline-distance probe-up probe-down 2))
(list (ly:skyline-distance probe-up probe-down -0.5) (ly:skyline-distance probe-up probe-down 1/3))
(list (ly:skyline-distance probe-odd probe-many) (ly:skyline-distance probe-odd probe-many 0.2) (ly:skyline-distance probe-many probe-odd 0.2))
(list (ly:skyline-distance probe-right probe-left) (ly:skyline-distance probe-right probe-left 0.25) (ly:skyline-distance probe-left probe-right 0.25))
(list (ly:skyline-distance probe-gon-up probe-gon-down) (ly:skyline-distance probe-gon-down probe-gon-up 0.1) (ly:skyline-distance probe-gon-up probe-many) (ly:skyline-distance probe-gon-up probe-many 0.4))
(list (ly:skyline-distance probe-floor probe-ceiling) (ly:skyline-distance probe-floor probe-ceiling 1) (ly:skyline-distance probe-up probe-ceiling) (ly:skyline-distance probe-ceiling probe-up 0.5))
(list (ly:skyline-distance probe-empty-up probe-down) (ly:skyline-distance probe-up probe-empty-down) (ly:skyline-distance probe-empty-up probe-empty-down 0.5) (ly:skyline-distance probe-floor probe-empty-down))
(list (ly:skyline-distance (ly:skyline-merge probe-up probe-odd) (ly:skyline-pad probe-down 0.3) 0.2) (ly:skyline-distance (ly:skyline-pad probe-gon-up 0.1) (ly:skyline-merge probe-gon-down probe-many)))
;; where they touch, and where each is highest (MS3 step 48)
(list (ly:skyline-touching-point probe-up probe-down) (ly:skyline-touching-point probe-down probe-up) (ly:skyline-touching-point probe-up probe-down 0.5) (ly:skyline-touching-point probe-down probe-up 0.5) (ly:skyline-touching-point probe-up probe-down 2))
(list (ly:skyline-touching-point probe-odd probe-many) (ly:skyline-touching-point probe-odd probe-many 0.2) (ly:skyline-touching-point probe-many probe-odd 0.2) (ly:skyline-touching-point probe-right probe-left) (ly:skyline-touching-point probe-left probe-right 0.25))
(list (ly:skyline-touching-point probe-gon-up probe-gon-down) (ly:skyline-touching-point probe-gon-down probe-gon-up 0.1) (ly:skyline-touching-point probe-gon-up probe-many 0.4) (ly:skyline-touching-point probe-floor probe-ceiling) (ly:skyline-touching-point probe-ceiling probe-up 0.5))
(list (ly:skyline-touching-point probe-empty-up probe-down) (ly:skyline-touching-point probe-up probe-empty-down) (ly:skyline-touching-point probe-empty-up probe-empty-down 0.5) (ly:skyline-touching-point probe-floor probe-empty-down))
(list (ly:skyline-max-height-position probe-up) (ly:skyline-max-height-position probe-down) (ly:skyline-max-height-position probe-odd) (ly:skyline-max-height-position probe-left) (ly:skyline-max-height-position probe-right))
(list (ly:skyline-max-height-position probe-empty-up) (ly:skyline-max-height-position probe-empty-down) (ly:skyline-max-height-position probe-floor) (ly:skyline-max-height-position probe-ceiling) (ly:skyline-max-height-position probe-gon-up) (ly:skyline-max-height-position probe-gon-down) (ly:skyline-max-height-position probe-many))
(list (ly:skyline-max-height-position (ly:skyline-pad probe-up 0.25)) (ly:skyline-max-height-position (ly:skyline-merge probe-gon-down probe-many)))
))

;;; The Scheme face (MS3 step 48).
;; skyline-scheme.cc's checks of its arguments as LY_ASSERT_SMOB and
;; LY_ASSERT_TYPE throw them, ly:make-skyline's of its segments before its
;; axis and direction, ly:skyline-merge's of the sides, and
;; ly:skylines-for-stencil on a stencil of each kind of expression
;; stencil-integral.cc walks, along either horizon axis. LilyPond 2.26.0
;; and the engine's (lily) run them (tools/bench/test/skylines.test.mjs);
;; crates/skyline's port has no Scheme, so tests/skyline.rs, which reads
;; only `(for-each probe-run '(…))`, leaves them out. A pair of skylines is
;; written as the points of its two, below (left) first.
(define (skyline-face-points p a)
  (list (ly:skyline->points (car p) a) (ly:skyline->points (cdr p) a)))
(define (skyline-face-stencil stencil)
  (list (skyline-face-points (ly:skylines-for-stencil stencil X) X)
        (skyline-face-points (ly:skylines-for-stencil stencil Y) Y)))
(define skyline-face-calls '(
;; the checks of each argument
(ly:skyline-height 5 1)
(ly:skyline-height probe-up 'a)
(ly:skyline-distance probe-up probe-down 'a)
(ly:skyline-distance probe-up 5)
(ly:skyline-touching-point probe-up probe-down "1")
(ly:skyline-touching-point #f probe-down)
(ly:skyline-max-height '())
(ly:skyline-max-height-position 1/2)
(ly:skyline-empty? (cons probe-down probe-up))
(ly:skyline-pad probe-up "1")
(ly:skyline-pad 0 1)
(ly:skyline->points probe-up 2)
(ly:skyline-merge probe-up probe-down)
(ly:skyline-merge probe-up 'up)
(ly:skylines-for-stencil 5 X)
(ly:skylines-for-stencil empty-stencil 3)
(list (ly:skyline? probe-up) (ly:skyline? 5) (ly:skyline? (cons probe-down probe-up)) (ly:skyline? (ly:make-skyline '() Y DOWN)))
;; ly:make-skyline's checks, and its exact and infinite numbers
(ly:make-skyline 5 X UP)
(ly:make-skyline '(5) X UP)
(ly:make-skyline '((0 0 1)) X UP)
(ly:make-skyline '((0 0 1 2 3)) X UP)
(ly:make-skyline '((0 0 1 a)) X UP)
(ly:make-skyline '((0 0 +inf.0 1)) X UP)
(ly:make-skyline '((-inf.0 0 1 1)) X UP)
(ly:make-skyline '((0 0 1 a)) 2 UP)
(ly:make-skyline '() 2 UP)
(ly:make-skyline '() X 2)
(ly:make-skyline '() X 'up)
(ly:skyline->points (ly:make-skyline '((-inf.0 1 0 1) (0 2 +inf.0 2)) Y DOWN) Y)
(ly:skyline->points (ly:make-skyline (list (list 0 0 1 1) (list 1/2 2 3/2 0)) X UP) X)
(ly:skyline->points (ly:make-skyline '((0 0 1 1) (0 0 1 1)) Y UP) X)
;; the stencils' skylines: lines, boxes square and round, circles, ellipses
;; and their arcs, paths of lines and curves, polygons, the rotated,
;; translated, scaled and combined, the empty, and those of no expression
;; the walk knows, which have their extents' box. A path is its expression:
;; make-path-stencil's extents need ly:bezier-extent, which no part of MS3
;; ports.
(skyline-face-stencil (make-line-stencil 0.1 0 0 2 1))
(skyline-face-stencil (make-line-stencil 0 -1 0.5 1 -0.5))
(skyline-face-stencil (make-filled-box-stencil '(0 . 2) '(-1 . 1)))
(skyline-face-stencil (ly:round-filled-box '(0 . 3) '(0 . 2) 0.5))
(skyline-face-stencil (ly:round-filled-box '(-1 . 1) '(-0.25 . 0.25) 0.6))
(skyline-face-stencil (make-circle-stencil 1 0.1 #f))
(skyline-face-stencil (make-circle-stencil 0.75 0 #t))
(skyline-face-stencil (make-ellipse-stencil 2 1 0.1 #t))
(skyline-face-stencil (make-oval-stencil 1.5 0.5 0.2 #f))
(skyline-face-stencil (make-partial-ellipse-stencil 2 1 0 180 0.1 #f #f))
(skyline-face-stencil (make-partial-ellipse-stencil 1 1.5 -45 200 0.2 #t #t))
(skyline-face-stencil (ly:make-stencil '(path 0.1 (moveto 0 0 curveto 1 1 2 1 3 0 closepath) round round #f) '(-0.05 . 3.05) '(-0.05 . 0.8)))
(skyline-face-stencil (ly:make-stencil '(path 0.2 (rmoveto 0.5 0 rlineto 1 2 rcurveto 0 1 1 1 2 0 lineto 0 -1 closepath) butt miter #t) '(-0.1 . 3.6) '(-1.1 . 3.1)))
(skyline-face-stencil (make-connected-path-stencil '((1 1) (2 0) (1 -1)) 0.1 1 1 #t #f))
(skyline-face-stencil (ly:round-polygon '((0 . 0) (2 . 0) (1 . 1.5)) 0.2))
(skyline-face-stencil (ly:round-polygon '((0 . 0) (2 . 1) (1 . 3) (-1 . 1)) 0.1 0.3 #f))
(skyline-face-stencil (ly:stencil-rotate (make-filled-box-stencil '(0 . 2) '(0 . 1)) 30 0 0))
(skyline-face-stencil (ly:stencil-rotate (make-line-stencil 0.2 0 0 2 0) 90 -1 1))
(skyline-face-stencil (ly:stencil-translate (make-circle-stencil 1 0.1 #f) '(1 . 2)))
(skyline-face-stencil (ly:stencil-scale (make-filled-box-stencil '(0 . 1) '(0 . 1)) 2 -0.5))
(skyline-face-stencil (ly:stencil-add (make-line-stencil 0.1 0 0 1 1) (make-filled-box-stencil '(2 . 3) '(-1 . 0))))
(skyline-face-stencil empty-stencil)
(skyline-face-stencil (ly:make-stencil '() '(0 . 1) '(0 . 1)))
(skyline-face-stencil (ly:make-stencil '(embedded-ps "0 0 moveto") '(0 . 2) '(-1 . 1)))
(skyline-face-stencil (ly:make-stencil '(embedded-ps "0 0 moveto") '(0 . 2) '(1 . -1)))
))
(for-each probe-run skyline-face-calls)

;;; The grobs' default skylines (MS3 step 26).
;; The skylines a fixture's grobs have by default, as ly:grob-property
;; 'vertical-skylines and 'horizontal-skylines give them: the grob's
;; definition's callback or the constructor's (grob.cc:79-88). Run by the
;; host in its Score's finalize, after the probe's text from this section's
;; heading on is evaluated in (lily): by tools/compare/skyline-calls.ly in
;; LilyPond 2.26.0, and by the engine from its image natively and in
;; core.wasm (tools/bench/test/skylines.test.mjs), on the same fixture.
;; (skyline-grobs-run heard) takes the grobs heard in the Score's order and
;; writes, for each of them of skyline-grobs-types, two lines: its type,
;; its number among the grobs of its type heard, the property, then `down`
;; and the points of the skyline below (left), `up` and those above
;; (right), along the horizon of the property's axis (X for the vertical
;; skylines, Y for the horizontal), each number as `write` writes it; or
;; `none` and the value as `write` writes it, when it is no pair of
;; skylines; or `error` and the key of what was thrown.
(define skyline-grobs-types '(Fingering StringNumber StrokeFinger DynamicText))
(define (skyline-grobs-line type n property)
  (lambda (grob)
    (display type) (display " ") (display n) (display " ") (display property)
    (catch #t
      (lambda ()
        (let ((skyp (ly:grob-property grob property))
              (horizon (if (eq? property 'vertical-skylines) X Y)))
          (if (and (pair? skyp) (ly:skyline? (car skyp)) (ly:skyline? (cdr skyp)))
              (for-each (lambda (side sky)
                          (display " ") (display side)
                          (for-each (lambda (point)
                                      (display " ") (write (car point))
                                      (display " ") (write (cdr point)))
                                    (ly:skyline->points sky horizon)))
                        '(down up) (list (car skyp) (cdr skyp)))
              (begin (display " none ") (write skyp)))))
      (lambda (key . args) (display " error ") (display key)))
    (newline)))
(define (skyline-grobs-run heard)
  (let loop ((heard heard) (counts '()))
    (if (pair? heard)
        (let* ((grob (car heard))
               (type (grob::name grob))
               (n (or (assq-ref counts type) 0)))
          (when (memq type skyline-grobs-types)
            ((skyline-grobs-line type n 'vertical-skylines) grob)
            ((skyline-grobs-line type n 'horizontal-skylines) grob))
          (loop (cdr heard) (assq-set! (alist-copy counts) type (1+ n)))))))
