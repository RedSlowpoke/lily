\version "2.26.0"
% Grobs that Scheme engravers make with ly:engraver-make-grob (MS3 step
% 12), for grobs.ly's capture and grobs.mjs' compare. Each Voice's
% engraver makes a Fingering of each note event, a grace note's and a
% chord's among them; each Staff's makes a StringNumber of each Fingering
% it hears; the Score's makes a StrokeFinger of each StringNumber, whose
% link is reached through two grob causes, and one with no cause at the
% first timestep. No type is one of part 2's 34, so that these lines
% compare before part 2's engravers are ported. Two staves and a named
% voice give the contexts' names numbers and an id. Each grob gets its
% text, side-axis and direction, which the engravers of these types give
% them, so that the fixture lays out without a warning.
#(define (fixture-text grob text)
   (ly:grob-set-property! grob 'text text)
   (ly:grob-set-property! grob 'side-axis Y)
   (ly:grob-set-property! grob 'direction UP)
   grob)
#(define (fixture-voice context)
   (let ((heard '()))
     (make-engraver
      (listeners
       ((note-event engraver event)
        (set! heard (cons event heard))))
      ((process-music engraver)
       (for-each (lambda (event) (fixture-text (ly:engraver-make-grob engraver 'Fingering event) "1"))
                 (reverse heard))
       (set! heard '()))
      ((stop-translation-timestep engraver)
       (set! heard '())))))
#(define (fixture-staff context)
   (make-engraver
    (acknowledgers
     ((finger-interface engraver grob source-engraver)
      (fixture-text (ly:engraver-make-grob engraver 'StringNumber grob) "2")))))
#(define (fixture-score context)
   (let ((first #t))
     (make-engraver
      ((process-music engraver)
       (when first
         (set! first #f)
         (fixture-text (ly:engraver-make-grob engraver 'StrokeFinger '()) "p")))
      (acknowledgers
       ((string-number-interface engraver grob source-engraver)
        (fixture-text (ly:engraver-make-grob engraver 'StrokeFinger grob) "i"))))))
\layout {
  \context { \Voice \consists #fixture-voice }
  \context { \Staff \consists #fixture-staff }
  \context { \Score \consists #fixture-score }
}
\new Score <<
  \new Staff \new Voice = "one" \relative { c'4 \grace d8 e4 <f a>2 }
  \new Staff \relative { r4 g' r2 }
  % MS3 step 19: Dynamic_engraver's DynamicTexts of a third staff's rests
  % and a Hairpin from the first to the last; rests have no Fingering, and
  % the lines above keep their numbers, which the references' links name.
  \new Staff { r4\p\< r4 r2\f }
>>
