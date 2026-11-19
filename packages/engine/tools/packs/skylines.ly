\version "2.26.0"
%% W2's check (chapter 06 §9): every glyph's vertical skyline pair (horizon
%% axis X) as LilyPond 2.26 computes it from the outline, which
%% crates/skyline/tests/skylines_ly.rs compares with ours point for point.
%% Chapter 06 Appendix A's census, writing the points instead of their counts.
%%
%%   SKYLINES_OUT=<dir> lilypond -dno-print-pages skylines.ly
%%
%% writes into <dir>, for the music font at each of its eight optical sizes and
%% for the braces, as the layout gets them at the default staff size
%% (ly:paper-get-font; ly:skylines-for-stencil needs a font scaled by its
%% magnification):
%%   <font>.tsv     a line `<font> <magnification>`, then per glyph of
%%                  ly:otf-glyph-list whose stencil is not empty two lines,
%%                  `<name> down <x y …>` and `<name> up <x y …>`, the points
%%                  of ly:skyline->points
%%
%% The music font's sizes come from the font-size that asks for each design
%% size exactly, 6 log2 (size / 20); at font-size 0 that is emmentaler-20 and
%% its 663 drawn glyphs, the 663 skylines of chapter 06 §5.1.
%%
%% A number is written exactly, as extents.ly writes it: the rational
%% inexact->exact makes of the double, -0 for negative zero, and +inf.0 or
%% -inf.0 for an infinity.

#(define (exact-text x)
   (cond ((inf? x) (number->string x))
         ((eqv? x -0.0) "-0")
         (else (number->string (inexact->exact x)))))

#(define (points-text points)
   (string-join
     (append-map (lambda (p) (list (exact-text (car p)) (exact-text (cdr p))))
                 points)
     " "))

#(define (write-skylines font port)
   (for-each
     (lambda (name)
       (let ((stil (ly:font-get-glyph font name)))
         (unless (ly:stencil-empty? stil)
           (let ((pair (ly:skylines-for-stencil stil X)))
             (for-each
               (lambda (side sky)
                 (display (string-append name "\t" side "\t"
                                         (points-text (ly:skyline->points sky X))
                                         "\n")
                          port))
               '("down" "up")
               (list (car pair) (cdr pair)))))))
     (ly:otf-glyph-list font)))

#(define-markup-command (skylines layout props) ()
   (for-each
     (lambda (spec)
       (let* ((font (ly:paper-get-font
                      layout
                      (cons `((font-encoding . ,(car spec)) (font-size . ,(cdr spec)))
                            props)))
              (port (open-output-file
                      (string-append (getenv "SKYLINES_OUT") "/"
                                     (ly:font-file-name font) ".tsv"))))
         (display (ly:font-file-name font) port)
         (display "\t" port)
         (display (exact-text (ly:font-magnification font)) port)
         (newline port)
         (write-skylines font port)
         (close-port port)))
     (cons '(fetaBraces . 0)
           (map (lambda (size)
                  (cons 'fetaMusic
                        (if (= size 20) 0 (* 6 (/ (log (/ size 20)) (log 2))))))
                '(11 13 14 16 18 20 23 26))))
   empty-stencil)

\markup \skylines
