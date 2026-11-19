;;; probe-data.scm: run in (lily) once lily.scm's init data has loaded, from source or from an image
;; What color.scm, define-event-classes.scm, define-note-names.scm,
;; define-music-properties.scm, define-context-properties.scm, breath.scm,
;; script.scm, parser-clef.scm, layout-slur.scm, define-grob-properties.scm,
;; define-grobs.scm, define-grob-interfaces.scm and define-stencil-commands.scm
;; leave behind: their tables, the type of each property they define, each
;; grob's description and the interfaces, one line per entry (LilyPond's C++
;; registers more interfaces, which the test leaves out). A procedure is
;; written by its name, an unpure-pure container by its parts and a hash
;; table by its entries, sorted, so that LilyPond 2.26.0's C++ callbacks and
;; the engine's placeholders of them (D67) write the same.
;; tools/bench/test/image.test.mjs runs it in LilyPond and in a VM that loaded
;; the files from source, natively, and in ones restored from the image,
;; natively and in wasm, and the outputs must be the same.
(set-current-module (resolve-module '(lily)))
(define (probe-data x)
  (cond ((procedure? x)
         (let ((name (procedure-name x)))
           (if name (string-append "#<procedure " (symbol->string name) ">") "#<procedure>")))
        ((ly:unpure-pure-container? x)
         (string-append "#<unpure-pure " (probe-data (ly:unpure-pure-container-unpure-part x))
                        " " (probe-data (ly:unpure-pure-container-pure-part x)) ">"))
        ((pair? x)
         (let loop ((x x) (items '()))
           (cond ((pair? x) (loop (cdr x) (cons (probe-data (car x)) items)))
                 ((null? x) (string-append "(" (string-join (reverse items) " ") ")"))
                 (else (string-append "(" (string-join (reverse items) " ") " . " (probe-data x) ")")))))
        ((vector? x) (string-append "#" (probe-data (vector->list x))))
        ((hash-table? x) (string-append "#<hash-table " (probe-data (probe-sorted-entries x)) ">"))
        (else (object->string x))))
(define (probe-sorted-entries table)
  (sort (hash-map->list cons table)
        (lambda (a b) (string<? (object->string (car a)) (object->string (car b))))))
(define (probe-line . items)
  (display (string-join items " "))
  (newline))
(define (probe-name x) (object->string x))
(define (probe-entries title alist)
  (for-each (lambda (entry) (probe-line title (probe-name (car entry)) (probe-data (cdr entry)))) alist))

;; color.scm
(probe-entries "x11-color" x11-color-list)
(probe-entries "css-color" css-color-list)
(probe-entries "universal-color" universal-color-list)
(probe-line "standard-colors" (probe-data standard-color-list))
;; define-event-classes.scm
(probe-line "event-classes" (probe-data event-classes))
(probe-entries "event-class" all-event-classes)
(probe-line "ancestor-lookup" (probe-data ancestor-lookup))
;; define-note-names.scm
(probe-entries "language" language-pitch-names)
;; the properties
(define (probe-properties title type? symbols)
  (for-each
   (lambda (symbol)
     (probe-line title (probe-name symbol) (probe-data (object-property symbol type?))))
   symbols))
(probe-properties "music-property" 'music-type? all-music-properties)
(probe-properties "translation-property" 'translation-type? all-translation-properties)
(probe-line "default-melisma-properties" (probe-data default-melisma-properties))
(probe-properties "backend-property" 'backend-type? all-backend-properties)
(probe-line "user-grob-properties" (probe-data (length all-user-grob-properties)))
(probe-line "internal-grob-properties" (probe-data (length all-internal-grob-properties)))
;; breath.scm, script.scm, parser-clef.scm, layout-slur.scm
(probe-entries "breath" default-breath-alist)
(probe-entries "script" default-script-alist)
(probe-line "toe-heel" (probe-data toe-heel-styles) (probe-data toe-heel-glyphs))
;; Less its separators, and the clef tablature.scm adds, which belongs to
;; the fretted part (D74), as does its entry in c0-pitch-alist.
(probe-entries "clef" (filter (lambda (e) (and (pair? e) (not (equal? (car e) "moderntab"))))
                              supported-clefs))
(probe-entries "c0-pitch" (filter (lambda (e) (not (equal? (car e) "markup.moderntab")))
                                  c0-pitch-alist))
(probe-entries "slur-details" default-slur-details)
;; define-grobs.scm
(for-each
 (lambda (grob)
   (probe-line "grob" (probe-name (car grob)) (probe-data (object-property (car grob) 'is-grob?))
         (probe-data (object-property (car grob) 'translation-type?)))
   (probe-entries (probe-name (car grob)) (cdr grob)))
 all-grob-descriptions)
;; define-grob-interfaces.scm and define-stencil-commands.scm
(for-each (lambda (entry) (probe-line "interface" (probe-data (cdr entry))))
          (probe-sorted-entries (ly:all-grob-interfaces)))
(probe-line "stencil-expressions" (probe-data (ly:all-stencil-expressions)))
(probe-line "stencil-heads" (probe-data (map (lambda (s) (object-property s 'stencil-head?))
                                 (ly:all-stencil-expressions))))
