%% Lily Studio's Karaoke (DECISIONS D133): the words of a score's \midi blocks.
%% A performer in each Lyrics context records every syllable: its moment, its
%% text, whether a hyphen or an extender follows it, its line (the Lyrics
%% contexts numbered in the order they first sing), the voice it is sung to
%% and whether it is written \markup \italic (D138). A performer in Score writes
%% them to <output-name>.words.json, one array entry per \midi performance, in
%% the order of the entries of timing.ly's <output-name>.timing.json.
%%
%% This file is the studio's alone, and the words are a file of their own
%% because timing.ly's map is what the engine reproduces: nothing here needs
%% timing.ly, so the engine can run it beside its native map, and a score's
%% Scheme writing a file is the interface it has for that. Nothing here may
%% fail a compile: everything that writes is wrapped, and an error only loses
%% the words.

#(begin
  (define lily-words-performances '())
  (define lily-words-output-name #f)
  (define lily-words-syllables '())
  (define lily-words-lines '())

  (define (lily-words-json-string text)
    (string-append
     "\""
     (string-concatenate
      (map (lambda (char)
             (case char
               ((#\") "\\\"")
               ((#\\) "\\\\")
               ((#\newline) "\\n")
               ((#\return) "\\r")
               ((#\tab) "\\t")
               (else (if (< (char->integer char) 32)
                         (format #f "\\u~4,'0x" (char->integer char))
                         (string char)))))
           (string->list text)))
     "\""))

  (define (lily-words-json-number value)
    (let ((inexact (exact->inexact value)))
      (if (and (integer? inexact) (< (abs inexact) 1e15))
          (number->string (inexact->exact inexact))
          (number->string inexact))))

  (define (lily-words-text value)
    (cond ((string? value) value)
          ((markup? value) (markup->string value))
          (else "")))

  ;; A markup with \italic in it. It walks car and cdr, as a markup may hold
  ;; a dotted pair: (baseline-skip . 3), say.
  (define (lily-words-italic? value)
    (and (pair? value)
         (or (eq? (car value) italic-markup)
             (lily-words-italic? (car value))
             (lily-words-italic? (cdr value)))))

  (define (lily-words-syllable->json syllable)
    (format #f "{\"at\":~a,\"text\":~a,\"hyphen\":~a,\"extender\":~a,\"line\":~a,\"voice\":~a,\"italic\":~a}"
            (lily-words-json-number (list-ref syllable 0))
            (lily-words-json-string (list-ref syllable 1))
            (if (list-ref syllable 2) "true" "false")
            (if (list-ref syllable 3) "true" "false")
            (list-ref syllable 4)
            (lily-words-json-string (list-ref syllable 5))
            (if (list-ref syllable 6) "true" "false")))

  (define (lily-words-write!)
    (let ((name (ly:parser-output-name)))
      (unless (equal? name lily-words-output-name)
        (set! lily-words-output-name name)
        (set! lily-words-performances '()))
      (set! lily-words-performances
            (append lily-words-performances (list (reverse lily-words-syllables))))
      (with-output-to-file (string-append (basename name) ".words.json")
        (lambda ()
          (display "[")
          (display (string-join
                    (map (lambda (syllables)
                           (string-append
                            "["
                            (string-join (map lily-words-syllable->json syllables) ",")
                            "]"))
                         lily-words-performances)
                    ","))
          (display "]\n")))))

  (define (Lily_words_performer context)
    (make-performer
     (listeners
      ((lyric-event performer event)
       (catch #t
         (lambda ()
           (let* ((posts (ly:event-property event 'articulations '()))
                  (post? (lambda (class)
                           (any (lambda (post) (ly:in-event-class? post class)) posts))))
             (unless (assq context lily-words-lines)
               (set! lily-words-lines
                     (acons context (length lily-words-lines) lily-words-lines)))
             (set! lily-words-syllables
                   (cons (list (ly:moment-main (ly:context-current-moment context))
                               (lily-words-text (ly:event-property event 'text))
                               (post? 'hyphen-event)
                               (post? 'extender-event)
                               (assq-ref lily-words-lines context)
                               (let ((voice (ly:context-property context 'associatedVoiceContext #f)))
                                 (or (and (ly:context? voice) (ly:context-id voice)) ""))
                               (lily-words-italic? (ly:event-property event 'text)))
                         lily-words-syllables))))
         (lambda (key . args)
           (ly:warning "words: ~a ~a" key args)))))))

  (define (Lily_words_score_performer context)
    (make-performer
     ((initialize performer)
      (set! lily-words-syllables '())
      (set! lily-words-lines '()))
     ((finalize performer)
      (catch #t
        lily-words-write!
        (lambda (key . args)
          (ly:warning "could not write the words: ~a ~a" key args))))))
)

\midi {
  \context {
    \Score
    \consists #Lily_words_score_performer
  }
  \context {
    \Lyrics
    \consists #Lily_words_performer
  }
}
