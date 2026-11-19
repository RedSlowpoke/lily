%% The references of W3 and text (06 §3.3, MS3 step 6): each page's stencil as
%% framework-svg.scm's output-stencils receives it, written before LilyPond
%% writes the page, into PAGES_OUT, and every top-level interpretation of a
%% markup made while the pages are built, with its markup, its property chain
%% and its stencil, into MARKUPS_OUT. capture.ly includes it; capture.mjs sets
%% both per score. Nothing it does changes the pages.
%%
%% A top-level interpretation is one made outside any other: interpret-markup
%% (titling.scm's titles, headers and footers with the tagline),
%% grob-interpret-markup (time signatures and the like) and a grob's stencil
%% ly:text-interface::print (TextScripts, bar numbers and the like, read from
%% the grob descriptions); what a markup command interprets inside it is part
%% of its stencil. Interpretations C++ makes itself (a volta bracket's number)
%% are not seen.
%%
%% Each file holds one datum per record, written as data the engine's reader
%% reads:
%%
%%   (page <file> <number> <X extent> <Y extent> <expression>)
%%   (markup <number> <source> <markup> <property chain> <X extent> <Y extent> <expression>)
%%
%% <source> is (interpret-markup), (grob-interpret-markup <grob> <link>) or
%% (print <grob> <link>), <grob> its name and <link> its cause's textedit link
%% or #f. In values, what is not data is written by a list headed by a symbol
%% of its own:
%%
%%   (%font <file name> <name> <magnification>)  a font: ly:font-file-name (#f
%%                                       for Pango's), ly:font-name and
%%                                       ly:font-magnification
%%   (%procedure <name>)                 a procedure, a markup command among
%%                                       them, by procedure-name (#f if none)
%%   (%grob <name>)                      a grob
%%   (%object <type>)                    anything else, by the type its
%%                                       printed form names
%%
%% and an expression's (grob-cause <grob> <expression>) is written
%% (grob-cause <link> <expression>), the link as output-svg.scm's grob-cause
%% writes it (textedit://<file>:<line>:<char>:<column + 1>), or #f where it
%% writes none.
#(begin
  (define (stencils-open var)
    (let ((port (open-file (or (getenv var) (string-append (string-downcase var) ".scm")) "w")))
      (set-port-encoding! port "UTF-8")
      port))
  (define stencils-pages-port (stencils-open "PAGES_OUT"))
  (define stencils-markups-port (stencils-open "MARKUPS_OUT"))
  (define (stencils-record port datum)
    (write datum port)
    (newline port)
    (force-output port))

  ;; output-svg.scm's grob-cause, without its side effect: the link or #f.
  (define (stencils-link grob)
    (let* ((cause (ly:grob-property grob 'cause))
           (origin (and (ly:stream-event? cause) (ly:event-property cause 'origin))))
      (and (ly:input-location? origin)
           (let* ((location (ly:input-file-line-char-column origin))
                  (raw-file (car location))
                  (file (if (is-absolute? raw-file) raw-file (string-append (ly-getcwd) "/" raw-file))))
             (format #f "textedit://~a:~a:~a:~a"
                     (ly:string-percent-encode (ly:string-substitute "\\" "/" file))
                     (cadr location) (caddr location) (1+ (cadddr location)))))))

  ;; The type a value's printed form names: #<Type …> or #<type …>.
  (define (stencils-type x)
    (let* ((s (format #f "~a" x))
           (end (string-index s (char-set #\space #\> #\:) 2)))
      (if (string-prefix? "#<" s) (substring s 2 (or end (string-length s))) s)))

  (define (stencils-datum x)
    (cond ((or (number? x) (string? x) (symbol? x) (boolean? x) (char? x) (null? x) (keyword? x)) x)
          ((and (pair? x) (eq? (car x) 'grob-cause) (pair? (cdr x)) (ly:grob? (cadr x)) (pair? (cddr x)))
           (list 'grob-cause (stencils-link (cadr x)) (stencils-datum (caddr x))))
          ((pair? x)
           ;; The spine iteratively, so that a long list does not nest the recursion.
           (let loop ((x x) (acc '()))
             (if (pair? x)
                 (loop (cdr x) (cons (stencils-datum (car x)) acc))
                 (append-reverse! acc (if (null? x) '() (stencils-datum x))))))
          ((vector? x) (list->vector (map stencils-datum (vector->list x))))
          ((ly:font-metric? x)
           (list '%font (ly:font-file-name x) (ly:font-name x) (ly:font-magnification x)))
          ((procedure? x) (list '%procedure (procedure-name x)))
          ((ly:grob? x) (list '%grob (grob::name x)))
          (else (list '%object (stencils-type x)))))

  (define (stencils-extents stencil)
    (list (ly:stencil-extent stencil X) (ly:stencil-extent stencil Y)))

  ;; A property chain, each \header's alist sorted by name (MS3 step 12a).
  ;; titling.scm's headers-property-alist-chain makes it with
  ;; ly:module->alist, a fold over the module's hash table, whose order
  ;; follows the symbols' addresses and so changes from run to run. A lookup
  ;; finds a key by name, and a module's names are unique. The alist of
  ;; marked-up-headfoot's literal properties, page: keys among its header:
  ;; ones, keeps its order.
  (define (stencils-header-alist? alist)
    (and (pair? alist)
         (every (lambda (entry)
                  (and (pair? entry) (symbol? (car entry)) (string-prefix? "header:" (symbol->string (car entry)))))
                alist)))
  (define (stencils-props props)
    (if (list? props)
        (map (lambda (alist)
               (if (stencils-header-alist? alist)
                   (sort alist (lambda (a b) (string<? (symbol->string (car a)) (symbol->string (car b)))))
                   alist))
             props)
        props))

  ;; The pages: output-stencils looked up by name in framework-svg (paper-book.cc).
  (let* ((framework (resolve-module '(lily framework-svg)))
         (output-stencils (module-ref framework 'output-stencils)))
    (module-set! framework 'output-stencils
      (lambda (name stencils header paper formats)
        (let ((first (1- (ly:output-def-lookup paper 'first-page-number)))
              (base (basename name)))
          (for-each
           (lambda (page i)
             (stencils-record stencils-pages-port
               (cons* 'page
                      (format #f "~a~a.svg" base (if (= (length stencils) 1) "" (format #f "-~a" (+ first i))))
                      (+ first i)
                      (append (stencils-extents page) (list (stencils-datum (ly:stencil-expr page)))))))
           stencils (iota (length stencils) 1)))
        (output-stencils name stencils header paper formats))))

  ;; The markups: depth counts interpretations under way, so only the top level is written.
  (define stencils-depth 0)
  (define stencils-count 0)
  (define (stencils-markup source markup props interpret)
    (if (> stencils-depth 0)
        (interpret)
        (let ((stencil (dynamic-wind
                        (lambda () (set! stencils-depth (1+ stencils-depth)))
                        interpret
                        (lambda () (set! stencils-depth (1- stencils-depth))))))
          (set! stencils-count (1+ stencils-count))
          (stencils-record stencils-markups-port
            (cons* 'markup stencils-count source (stencils-datum markup) (stencils-datum (stencils-props props))
                   (if (ly:stencil? stencil)
                       (append (stencils-extents stencil) (list (stencils-datum (ly:stencil-expr stencil))))
                       (list #f #f (stencils-datum stencil)))))
          stencil)))
  ;; Font_interface::text_font_alist_chain.
  (define (stencils-chain grob) (ly:grob-alist-chain grob '((font-encoding . latin1))))
  (define (stencils-grob what grob) (list what (grob::name grob) (stencils-link grob)))

  (define stencils-lily (resolve-module '(lily)))
  (let ((interpret interpret-markup))
    (module-set! stencils-lily 'interpret-markup
          (lambda (layout props markup)
            (stencils-markup '(interpret-markup) markup props (lambda () (interpret layout props markup))))))
  (let ((interpret grob-interpret-markup))
    (module-set! stencils-lily 'grob-interpret-markup
          (lambda (grob markup)
            (stencils-markup (stencils-grob 'grob-interpret-markup grob) markup (stencils-chain grob)
                             (lambda () (interpret grob markup))))))
  (define (stencils-print grob)
    (stencils-markup (stencils-grob 'print grob) (ly:grob-property grob 'text) (stencils-chain grob)
                     (lambda () (ly:text-interface::print grob))))
  (define stencils-grob-descriptions
    (map (lambda (description)
           (cons (car description)
                 (map (lambda (entry)
                        (if (and (eq? (car entry) 'stencil) (eq? (cdr entry) ly:text-interface::print))
                            (cons 'stencil stencils-print)
                            entry))
                      (cdr description))))
         all-grob-descriptions)))
\layout { \context { \Global \grobdescriptions #stencils-grob-descriptions } }
