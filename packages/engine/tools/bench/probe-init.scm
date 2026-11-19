;;; probe-init.scm: run in (lily) once declarations-init.ly has been parsed and run, from source or from an image
;; One line per variable the whole init recorded (MS2 step 16), by name:
;; its name and what its value is. A music function's is its signature, each
;; predicate by its procedure name, an optional argument's (predicate .
;; default) with its default as write writes it, and music, the result's
;; default among them, as (music name). Music is (music name), a procedure
;; (procedure name), an output definition its kind, a context definition
;; or modification its class; a number, a string, a symbol or a boolean is
;; written; anything else is its class.
;; tools/bench/test/image.test.mjs runs it in a VM that loaded the files from
;; source and in ones restored from the image, natively and in wasm, and the
;; outputs must be the same, and LilyPond 2.26.0's but the variables of
;; define-session-public, which LilyPond records in the init's module too.
(set-current-module (resolve-module '(lily)))
(define (signature-names x)
  (cond ((procedure? x) (procedure-name x))
        ((ly:music? x) (list 'music (ly:music-property x 'name)))
        ((pair? x) (cons (signature-names (car x)) (signature-names (cdr x))))
        (else x)))
(define (value-kind v)
  (cond ((ly:music-function? v) (signature-names (ly:music-function-signature v)))
        ((ly:music? v) (list 'music (ly:music-property v 'name)))
        ((procedure? v) (list 'procedure (procedure-name v)))
        ((ly:output-def? v) (list 'output-def (ly:output-def-lookup v 'output-def-kind)))
        ((ly:context-def? v) 'context-def)
        ((ly:context-mod? v) 'context-mod)
        ((ly:duration? v) (list 'duration (ly:duration->string v)))
        ((ly:pitch? v) (list 'pitch (ly:pitch-notename v) (ly:pitch-alteration v) (ly:pitch-octave v)))
        ((ly:moment? v) 'moment)
        ((or (number? v) (string? v) (symbol? v) (boolean? v) (null? v)) v)
        ((hash-table? v) 'hash-table)
        ((pair? v) 'pair)
        ((vector? v) 'vector)
        ((module? v) 'module)
        (else 'other)))
(for-each
 (lambda (record)
   (write (car record))
   (display " ")
   (write (value-kind (cdddr record)))
   (newline))
 (sort (filter cadr lilypond-declarations)
       (lambda (a b) (symbol<? (car a) (car b)))))
