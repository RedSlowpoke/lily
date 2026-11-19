;;; probe-columns.scm: the paper columns' skylines (MS3 step 30)
;; The skylines of a score's paper columns, as ly:grob-property
;; 'horizontal-skylines gives them: ly:separation-item::calc-skylines over
;; the boxes of each column's elements, which Spacing_interface's and
;; Paper_column's minimum distances read (they have no Scheme face in
;; 2.26.0, and the engine has no ly:skyline-distance; crates/layout's
;; tests/layout.rs checks them). Run by the host in its Score's finalize,
;; after this file is evaluated in (lily): by tools/compare/columns-calls.ly
;; in LilyPond 2.26.0, and by the engine from its image natively and in
;; core.wasm (tools/bench/test/columns.test.mjs), on the same fixture
;; (tools/compare/fixtures/columns.ly), which compares them to four
;; decimals.
;;
;; (columns-run columns) takes the columns the Score's
;; currentCommandColumn and currentMusicalColumn named, in the order they
;; were first named, and writes a line for each: its type, its number
;; among the columns of its type, `horizontal-skylines`, then `left` and
;; the points of the skyline left of it, `right` and those right of it,
;; along Y, each number as `write` writes it; or `none` and the value as
;; `write` writes it, when it is no pair of skylines; or `error` and the
;; key of what was thrown.
(set-current-module (resolve-module '(lily)))
(define (columns-line type n column)
  (display type) (display " ") (display n) (display " horizontal-skylines")
  (catch #t
    (lambda ()
      (let ((skyp (ly:grob-property column 'horizontal-skylines)))
        (if (and (pair? skyp) (ly:skyline? (car skyp)) (ly:skyline? (cdr skyp)))
            (for-each (lambda (side sky)
                        (display " ") (display side)
                        (for-each (lambda (point)
                                    (display " ") (write (car point))
                                    (display " ") (write (cdr point)))
                                  (ly:skyline->points sky Y)))
                      '(left right) (list (car skyp) (cdr skyp)))
            (begin (display " none ") (write skyp)))))
    (lambda (key . args) (display " error ") (display key)))
  (newline))
(define (columns-run columns)
  (let loop ((columns columns) (counts '()))
    (if (pair? columns)
        (let* ((column (car columns))
               (type (grob::name column))
               (n (or (assq-ref counts type) 0)))
          (columns-line type n column)
          (loop (cdr columns) (assq-set! (alist-copy counts) type (1+ n)))))))
