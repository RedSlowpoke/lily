\version "2.26.0"
% The columns' fixture (MS3 step 30), for columns-calls.ly's probe of the
% paper columns' skylines: two staves whose non-musical columns hold bar
% lines and time signatures, placed in the column by its break alignment,
% and whose musical columns hold note names, one or two a column, made of
% skips by a Voice engraver, some widened, raised, sized or aligned. The
% \layout removes what path.ly's removes (the engine does not draw clefs,
% keys, ledger lines, delimiters or bar numbers yet), and the
% Vertical_align_engraver: the pure Y offset of a staff in an alignment
% reads the line's break ranks, which LilyPond has only once its items
% are broken and the engine only with breaking (MS3 step 32); with no
% alignment the staves' offsets are 0, and a column's skylines can be read
% when the Score is finalized, before either breaks. Notes are left out:
% the engine draws no note head, rest or stem before part 4.
#(define fixture-names
   ;; Each name's text and the properties it is given.
   `(("c")
     ("dis" (font-size . 2))
     ("fes" (extra-spacing-width . (-0.5 . 0.75)))
     ("g" (extra-spacing-height . (-1 . 1.5)))
     ("ais" (self-alignment-X . ,LEFT))
     ("beses" (Y-offset . 2.5))
     ("h" (extra-spacing-width . (+inf.0 . -inf.0)))
     ("cis" (self-alignment-X . ,RIGHT) (font-size . -3))))
#(define (fixture-namer context)
   (let ((heard '()) (next 0))
     (make-engraver
      (listeners
       ((skip-event engraver event)
        (set! heard (cons event heard))))
      ((process-music engraver)
       (for-each
        (lambda (event)
          (let ((grob (ly:engraver-make-grob engraver 'NoteName event))
                (name (list-ref fixture-names (modulo next (length fixture-names)))))
            (ly:grob-set-property! grob 'text (car name))
            (for-each (lambda (property)
                        (ly:grob-set-property! grob (car property) (cdr property)))
                      (cdr name))
            (set! next (1+ next))))
        (reverse heard))
       (set! heard '()))
      ((stop-translation-timestep engraver)
       (set! heard '())))))
\layout {
  \context {
    \Voice
    \consists #fixture-namer
  }
  \context {
    \Staff
    \remove Clef_engraver
    \remove Key_engraver
    \remove Ledger_line_engraver
  }
  \context {
    \Score
    \remove Vertical_align_engraver
    \remove System_start_delimiter_engraver
    \remove Bar_number_engraver
  }
}
\score {
  <<
    \new Staff {
      \time 3/4 s2. | s4 s2 | s8 s4. s4 \bar "||"
      \time 2/4 s2 | s4 s8 s8 \bar ".|:" | s4 s4 \bar ":|." | s2 \bar "|."
    }
    \new Staff {
      \time 3/4 s2 s4 | s2. | s4 s4 s4 |
      \time 2/4 s4 s4 | s2 | s8 s8 s4 | s4. s8
    }
  >>
}
