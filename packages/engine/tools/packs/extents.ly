\version "2.26.0"
%% W1's check (chapter 06 §9): every glyph's extent as LilyPond 2.26 gives
%% it, which crates/fonts/tests/extents.rs compares with the packs of build.mjs.
%%
%%   EXTENTS_OUT=<dir> lilypond -dno-print-pages extents.ly
%%
%% writes into <dir>:
%%   <font>.tsv     emmentaler-11 … -26 and emmentaler-brace as
%%                  ly:system-font-load loads them: per glyph of ly:otf-glyph-list,
%%                  its name and the X and Y extents of ly:font-get-glyph
%%   staff-<encoding><font-size>.tsv
%%                  the same for the font the layout gets at the default staff
%%                  size (ly:paper-get-font), after a line `<font> <magnification>`
%%
%% A number is written exactly: the rational inexact->exact makes of the
%% double, -0 for negative zero, and +inf.0 or -inf.0 for an empty extent.

#(define (exact-text x)
   (cond ((inf? x) (number->string x))
         ((eqv? x -0.0) "-0")
         (else (number->string (inexact->exact x)))))

#(define (write-extents font port)
   (for-each
     (lambda (name)
       (let ((stil (ly:font-get-glyph font name)))
         (display (string-join
                    (cons name (map exact-text
                                    (let ((x (ly:stencil-extent stil X))
                                          (y (ly:stencil-extent stil Y)))
                                      (list (car x) (cdr x) (car y) (cdr y)))))
                    "\t")
                  port)
         (newline port)))
     (ly:otf-glyph-list font)))

#(define (out-file name)
   (string-append (getenv "EXTENTS_OUT") "/" name ".tsv"))

#(for-each
   (lambda (name)
     (let ((port (open-output-file (out-file name))))
       (write-extents (ly:system-font-load name) port)
       (close-port port)))
   '("emmentaler-11" "emmentaler-13" "emmentaler-14" "emmentaler-16"
     "emmentaler-18" "emmentaler-20" "emmentaler-23" "emmentaler-26"
     "emmentaler-brace"))

%% The music font at font-size 0 and -3 (grace notes, cue notes), and the braces.
#(define-markup-command (staff-extents layout props) ()
   (for-each
     (lambda (spec)
       (let* ((encoding (car spec))
              (size (cadr spec))
              (font (ly:paper-get-font
                      layout
                      (cons `((font-encoding . ,encoding) (font-size . ,size)) props)))
              (port (open-output-file
                      (out-file (format #f "staff-~a~a" encoding size)))))
         (display (ly:font-file-name font) port)
         (display "\t" port)
         (display (exact-text (ly:font-magnification font)) port)
         (newline port)
         (write-extents font port)
         (close-port port)))
     '((fetaMusic 0) (fetaMusic -3) (fetaBraces 0)))
   empty-stencil)

\markup \staff-extents
