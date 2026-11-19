;;; probe-select.scm: LilyPond's font selection (01 §3.7's O2, MS3 step 5)
;; Calls of ly:paper-get-font, run in (lily): by LilyPond 2.26.0 on its
;; OTFs (tools/compare/font-calls.ly with FONT_PROBE), and by the engine on
;; W1's packs, which the test puts first, from source with the library
;; natively, and from an image natively and in wasm
;; (tools/bench/test/fonts.test.mjs). Its harness defines in (lily)
;; `probe-paper`, the default \paper ($defaultpaper), and `probe-cases`,
;; what the 10 scores' grobs and markups selected
;; (tools/compare/font-select.ly): each case's staff-height, then its
;; chain's font-size, font-encoding, font-family and font-name, #f where
;; the chain has none. Each case is selected in a clone of probe-paper of
;; its staff-height, as layout-set-absolute-staff-size-in-module sets one,
;; with the chain the case's values then the paper's property-defaults; its
;; record is the font's file name, design size and magnification. Then the
;; calls written out below, and each staff size of the grid with each
;; font-size of the grid, fetaMusic, and fetaBraces at its own size: the
;; font, its names, design size and magnification, whether the next call
;; gives the same font, and a glyph's stencil from it. Text fonts are part
;; 1's step 8 and not called. The output is probe-fonts.scm's: for each
;; call a tab and its value as `write` writes it, a stencil as its
;; expression and extents, or the error's key, a tab and its text; then
;; the record separator (U+001E) and a newline.
(set-current-module (resolve-module '(lily)))
(define (probe-show x)
  (cond ((ly:stencil? x)
         (list 'stencil (ly:stencil-expr x) (ly:stencil-extent x 0) (ly:stencil-extent x 1)))
        ((pair? x) (cons (probe-show (car x)) (probe-show (cdr x))))
        (else x)))
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
(define probe-pt (ly:output-def-lookup probe-paper 'pt))
;; One paper per staff-height, so that a font's second selection finds the
;; scaled font its first made.
(define probe-papers '())
(define (probe-paper-at staff-height)
  (or (assoc-ref probe-papers staff-height)
      (let ((paper (ly:output-def-clone probe-paper)))
        (layout-set-absolute-staff-size-in-module (ly:output-def-scope paper) staff-height)
        (set! probe-papers (acons staff-height paper probe-papers))
        paper)))
(define (probe-chain paper size encoding family name)
  (list (append (if size (list (cons 'font-size size)) '())
                (if encoding (list (cons 'font-encoding encoding)) '())
                (if family (list (cons 'font-family family)) '())
                (if name (list (cons 'font-name name)) '()))
        (ly:output-def-lookup paper 'property-defaults)))
(define (probe-select staff-height size encoding family name)
  (let ((paper (probe-paper-at staff-height)))
    (ly:paper-get-font paper (probe-chain paper size encoding family name))))
;; A case of the scores: its font's file name, design size and magnification.
(define (probe-case case)
  (let ((font (apply probe-select case)))
    (list (ly:font-file-name font) (ly:font-design-size font) (ly:font-magnification font))))
;; A size of the grid: the font, its values and a glyph of it.
(define (probe-grid points size encoding glyph)
  (let* ((staff-height (* points probe-pt))
         (font (probe-select staff-height size encoding #f #f)))
    (list font (ly:font-file-name font) (ly:font-name font) (ly:font-design-size font)
          (ly:font-magnification font) (ly:font-metric? font) (ly:otf-font? font)
          (eq? font (probe-select staff-height size encoding #f #f))
          (ly:font-get-glyph font glyph))))
(for-each (lambda (case) (probe-run `(probe-case ',case))) probe-cases)
(for-each probe-run
  '((ly:paper-get-font 'paper '())
    (ly:paper-get-font probe-paper)
    (ly:otf-glyph-count (probe-select (* 20 probe-pt) -3 'fetaMusic #f #f))
    (length (ly:otf-glyph-list (probe-select (* 20 probe-pt) 0 'fetaBraces #f #f)))
    (ly:otf-font-glyph-info (probe-select (* 20 probe-pt) 2 'fetaMusic #f #f) "noteheads.s2")
    (ly:font-glyph-name-to-index (probe-select (* 20 probe-pt) 2 'fetaMusic #f #f) "noteheads.s2")
    (ly:font-index-to-charcode (probe-select (* 20 probe-pt) 2 'fetaMusic #f #f) 100)
    ;; A family the music encodings take as music's, and a size given as
    ;; no real.
    (probe-select (* 20 probe-pt) -2 'fetaMusic 'sans #f)
    (probe-select (* 20 probe-pt) "big" 'fetaMusic #f #f)
    ;; After ly:reset-all-fonts the font is loaded again, and its scaled
    ;; font is a new one.
    (let ((font (probe-select (* 20 probe-pt) 1 'fetaMusic #f #f)))
      (ly:reset-all-fonts)
      (list (eq? font (probe-select (* 20 probe-pt) 1 'fetaMusic #f #f))
            (equal? (ly:font-magnification font)
                    (ly:font-magnification (probe-select (* 20 probe-pt) 1 'fetaMusic #f #f)))))))
(for-each
 (lambda (points)
   (for-each (lambda (size) (probe-run `(probe-grid ,points ,size 'fetaMusic "noteheads.s2")))
             '(-6 -5 -4 -3.5 -3 -2 -1 0 0.5 1 2 3 4 6))
   (probe-run `(probe-grid ,points 0 'fetaBraces "brace100")))
 '(11 12.6 13 14 16 17.82 18 20 22.45 23 26 30))
