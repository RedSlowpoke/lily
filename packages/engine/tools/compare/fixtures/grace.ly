\version "2.26.0"
% N6's grace notes (MS2 step 35): \grace, \acciaccatura, \appoggiatura,
% \slashedGrace and \afterGrace (its fraction given and not), graces at the
% start, in a tuplet and in a chord's voice, and two voices whose graces
% fall at different times. Each \dir writes its Voice's Stem direction and
% font-size to dirs.txt, as graceSettings leave them (Grace_engraver), in
% \layout only.
#(define acc "")
dir = \applyContext #(lambda (c)
  (let ((stem (ly:context-grob-definition c 'Stem)))
    (if (pair? stem)
      (begin
        (set! acc (string-append acc
          (format #f "~a ~a ~a ~a\n" (ly:context-id c) (ly:context-current-moment c)
                  (assq-ref stem 'direction) (assq-ref stem 'font-size))))
        (with-output-to-file "dirs.txt" (lambda () (display acc)))))))
\new Staff <<
  \new Voice = "one" \relative {
    \dir \grace { \dir c'8 \dir d } \dir c4 \dir \acciaccatura e16 \dir d4
    \appoggiatura { f16 e } d4 \slashedGrace g8 f4 |
    \afterGrace c2 { d16 e } \afterGrace 7/8 f2 g8 |
    \tuplet 3/2 { c8 \grace b16 c8 d } \grace { \dir s16 } \dir c4 \dir r2 |
  }
  \new Voice = "two" \relative {
    \voiceTwo c'4 \dir c4 \dir \grace { \dir s16 } \dir c4 \dir c |
    << { c2 } \\ { \grace { a16 } b2 } >> c2 |
    R1 |
  }
>>
