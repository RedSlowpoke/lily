;;; probe-fonts.scm: LilyPond's font metrics over W1's packs (01 §3.7's O2, MS3 step 4)
;; Calls of font-metric-scheme.cc's and open-type-font-scheme.cc's ly:
;; functions on the fonts ly:system-font-load gives for emmentaler-20,
;; emmentaler-13 and emmentaler-brace, run in (lily): by LilyPond 2.26.0
;; from its OTFs (tools/compare/font-calls.ly), and by the engine from W1's
;; packs, which the test puts first (%put-font-pack), natively in a VM with
;; only the music crate's and layout's functions
;; (crates/layout/tests/fonts.rs), from source with the library natively,
;; and from an image natively and in wasm (tools/bench/test/fonts.test.mjs).
;; First the calls written out below, then, for each font, its own values
;; and every glyph of ly:otf-glyph-list by its name and its index: its
;; index, its character, its LILC entry and ly:font-get-glyph's stencil. The
;; functions that read what the packs do not hold (a table's bytes, a font
;; file) are the engine's placeholders, so only their argument errors are
;; called. For each call: a tab and its value as `write` writes it, a
;; stencil as its expression and extents, or the error's key, a tab, its
;; text as print-exception prints it and a newline; then the record
;; separator (U+001E) and a newline. Every run must give the same.
(set-current-module (resolve-module '(lily)))
(define (probe-show x)
  (cond ((ly:stencil? x)
         (list 'stencil (ly:stencil-expr x) (ly:stencil-extent x 0) (ly:stencil-extent x 1)))
        ((pair? x) (cons (probe-show (car x)) (probe-show (cdr x))))
        (else x)))
;; An error's text as Guile's print-exception prints a scm-error's, which
;; the engine has as Rust only: `In procedure subr: ` and the message.
(define (probe-error args)
  (let ((subr (car args)) (message (cadr args)) (items (caddr args)))
    (string-append (if subr (simple-format #f "In procedure ~A: " subr) "")
                   (apply simple-format #f message (if (list? items) items '())))))
(define (probe-run call)
  (catch #t
    (lambda ()
      (let ((value (primitive-eval call)))
        (display "\t")
        (write (probe-show value))))
    (lambda (key . args)
      (display key)
      (display "\t")
      (display (probe-error args))
      (newline)))
  (display (integer->char 30))
  (newline))
(define probe-fonts (map ly:system-font-load '("emmentaler-20" "emmentaler-13" "emmentaler-brace")))
(define probe-names (map ly:otf-glyph-list probe-fonts))
(define probe-f (car probe-fonts))
(define probe-b (caddr probe-fonts))
;; Font k's own values.
(define (probe-font k)
  (let ((f (list-ref probe-fonts k)))
    (list f (ly:font-name f) (ly:font-file-name f) (ly:font-design-size f)
          (ly:font-magnification f) (ly:font-metric? f) (ly:otf-font? f)
          (ly:otf-glyph-count f) (length (list-ref probe-names k)))))
;; Glyph i of font k, by its name and by its index.
(define (probe-glyph k i)
  (let ((f (list-ref probe-fonts k))
        (name (list-ref (list-ref probe-names k) i)))
    (list i name (ly:font-glyph-name-to-index f name) (ly:font-index-to-charcode f i)
          (ly:otf-font-glyph-info f name) (ly:font-get-glyph f name))))
(for-each probe-run '(
;; the predicates and the fonts loaded
(list (ly:font-metric? probe-f) (ly:font-metric? 1) (ly:font-metric? (ly:make-stencil '())) (ly:otf-font? probe-b) (ly:otf-font? "emmentaler-20"))
(list (eq? probe-f (ly:system-font-load "emmentaler-20")) (eq? probe-f probe-b) (equal? probe-f probe-f))
(ly:system-font-load 'emmentaler-20)
;; font-metric-scheme.cc's arguments
(ly:font-name 1)
(ly:font-file-name '(1 . 2))
(ly:font-magnification "emmentaler-20")
(ly:font-design-size 'x)
(ly:font-get-glyph 1 "noteheads.s2")
(ly:font-get-glyph probe-f 'noteheads.s2)
(ly:font-glyph-name-to-index probe-f 'a)
(ly:font-glyph-name-to-index (ly:make-stencil '()) "a")
(ly:font-index-to-charcode "x" 1)
(ly:font-index-to-charcode probe-f 'a)
(ly:font-index-to-charcode probe-f 1/2)
(ly:font-index-to-charcode probe-f 2.0)
(ly:font-index-to-charcode probe-f +inf.0)
(ly:font-index-to-charcode probe-f 2147483648)
(ly:font-index-to-charcode probe-f -2147483649)
(ly:font-index-to-charcode probe-f 100000000000000000000)
;; glyphs that are not, and names read with a - as M
(list (ly:font-get-glyph probe-f "nope") (ly:font-get-glyph probe-f "") (ly:font-get-glyph probe-f ".notdef") (ly:font-get-glyph probe-f "brace10"))
(list (ly:font-get-glyph probe-f "rests.-1") (ly:font-get-glyph probe-f "rests.-1-2") (ly:font-get-glyph probe-f "rests.M3") (ly:font-get-glyph probe-b "brace-1"))
(map (lambda (n) (ly:font-glyph-name-to-index probe-f n)) '("nope" "" ".notdef" "rests.-1" "rests.M1" "space" "brace10"))
(map (lambda (n) (ly:font-glyph-name-to-index probe-b n)) '(".notdef" "brace0" "brace575" "brace576" "space"))
(map (lambda (i) (ly:font-index-to-charcode probe-f i)) '(-1 -5 0 663 664 10000 2147483647 -2147483648))
(map (lambda (i) (ly:font-index-to-charcode probe-b i)) '(-1 0 1 576 577))
;; open-type-font-scheme.cc's arguments, and LILC's entries that are not
(ly:otf-font-glyph-info 1 "a")
(ly:otf-font-glyph-info probe-f 'a)
(map (lambda (n) (ly:otf-font-glyph-info probe-f n)) '("nope" ".notdef" "backslash" "rests.-1" "brace10"))
(ly:otf-font-glyph-info probe-b "brace0")
(ly:otf-font-table-data 1 "LILC")
(ly:otf-font-table-data probe-f 'LILC)
(ly:otf-font-table-data probe-f "LILCX")
(ly:otf-glyph-count 1)
(ly:otf-glyph-count probe-b)
(ly:otf-glyph-list "emmentaler-20")
(ly:get-font-format 1)
(ly:get-font-format "x" 'a)
(ly:get-font-format "x" 2.0)
(ly:get-font-format "x" 1/2)
(ly:has-glyph-names? 'x)
(ly:has-glyph-names? "x" 1.5)
(ly:has-glyph-names? "x" 4294967296)
(ly:get-cff-offset 1)
(ly:get-cff-offset "x" 'a)
(ly:get-cff-offset "x" -2147483649)
(ly:extract-subfont-from-collection 1 0 "x")
(ly:extract-subfont-from-collection "a" 'x "x")
(ly:extract-subfont-from-collection "a" 0 'x)
(ly:extract-subfont-from-collection "a" 'x 'y)
(ly:extract-subfont-from-collection "a" 2.0 'y)
(ly:extract-subfont-from-collection "a" 2.0 "x")
(ly:extract-subfont-from-collection "a" 2147483648 "x")
;; the glyph lists whole
(ly:otf-glyph-list probe-f)
(ly:otf-glyph-list (cadr probe-fonts))
(ly:otf-glyph-list probe-b)
))
;; Each font, then each of its glyphs.
(let font ((k 0))
  (if (< k (length probe-fonts))
      (begin
        (probe-run `(probe-font ,k))
        (let glyph ((i 0))
          (if (< i (length (list-ref probe-names k)))
              (begin
                (probe-run `(probe-glyph ,k ,i))
                (glyph (+ i 1)))))
        (font (+ k 1)))))
