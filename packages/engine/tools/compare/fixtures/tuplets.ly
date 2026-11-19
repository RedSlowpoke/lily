\version "2.26.0"
% TupletBracket and TupletNumber (MS4 step 12, for steps 13 and 14):
% brackets up and down by their stems and forced, over beams and without
% them, nested tuplets, a tuplet under a longer beam and a kneed one,
% brackets hidden and forced, tupletFullLength and tupletFullLengthNote, a
% tuplet across a bar line and one broken across a \break, rests at a
% tuplet's ends, a tuplet of one note and of none, scripts the brackets
% avoid, a fraction, a tuplet slur, two voices and a cross-staff tuplet.
% Brackets by the stems, over a beam (the number alone) and forced.
\score { \relative {
  \tuplet 3/2 { c''4 d e } \tuplet 3/2 { f,4 g a } |
  \tuplet 3/2 { b8 c d } \tuplet 3/2 { e8 f g } \tuplet 3/2 { c,,8 d e } c'4 |
  \tupletUp \tuplet 3/2 { g4 a b } \tupletDown \tuplet 3/2 { c4 d e } |
  \tupletNeutral \tuplet 3/2 { a,8 b c } r2. \bar "|."
} }
% Nested tuplets, and a tuplet of five over a sextuplet's beam.
\score { \relative {
  \tuplet 3/2 { \tuplet 3/2 { c''8 d e } f4 g } \tuplet 3/2 { a4 \tuplet 3/2 { g8 f e } d4 } |
  \tuplet 5/4 { c16 d e f g } \tuplet 6/4 { a16 g f e d c } \tuplet 3/2 { b4 \tuplet 5/4 { c16 d e f g } a4 } \bar "|."
} }
% A tuplet under a longer beam, sloped brackets, a kneed beam with an
% accidental beside its number, and wide intervals.
\score { \relative {
  c''8[ \tuplet 3/2 { d16 e f] } g8[ a] \tuplet 3/2 { c,4 g' d' } |
  \tuplet 3/2 { c,,8[ c'' c,,] } \tuplet 3/2 { e8[ fis'' g,,] } \tuplet 3/2 { a'4 e c } |
  \tuplet 3/2 { g4 c e } \tuplet 3/2 { c'4 g e } \bar "|."
} }
% Brackets hidden, forced over a beam, only without one; the bracket over
% the note heads.
\score { \relative {
  \override TupletBracket.bracket-visibility = ##f
  \tuplet 3/2 { c''4 d e } \tuplet 3/2 { f8 g a } r4 |
  \override TupletBracket.bracket-visibility = ##t
  \tuplet 3/2 { c8 b a } \tuplet 3/2 { g4 f e } r4 |
  \override TupletBracket.bracket-visibility = #'if-no-beam
  \tuplet 3/2 { d8 e f } \tuplet 3/2 { g4 a b } r4 |
  \revert TupletBracket.bracket-visibility
  \override TupletBracket.visible-over-note-heads = ##t
  \tupletDown \tuplet 3/2 { c8 d e } \tupletNeutral \tuplet 3/2 { f8 e d } r2 \bar "|."
} }
% tupletFullLength to the next note, and tupletFullLengthNote.
\score { \relative {
  \set tupletFullLength = ##t
  \tuplet 3/2 { c''4 d e } \tuplet 3/2 { f4 e d } |
  \set tupletFullLengthNote = ##t
  \tuplet 3/2 { c4 d e } \tuplet 3/2 { f4 g a } |
  \unset tupletFullLengthNote
  \tuplet 3/2 { g8 f e } d2. \bar "|."
} }
% A tuplet across a bar line, and one broken across a \break with its
% edge texts.
\score { \relative {
  c''2 \tuplet 3/2 { d2 e f } g2 |
  \override TupletBracket.edge-text = #(cons (markup #:arrow-head X LEFT #f)
                                              (markup #:arrow-head X RIGHT #f))
  c,2 \tuplet 3/2 { d2 e \break f } g2 | a1 \bar "|."
} }
% Rests at a tuplet's ends, a tuplet of one note, one of none.
\score { \relative {
  \tuplet 3/2 { r4 c'' d } \tuplet 3/2 { e4 f r } |
  \tuplet 3/2 { r4 g r } \tuplet 3/2 { s8 a s } \tuplet 3/2 { s8 s s } |
  \tuplet 3/2 { r8 b c } \tuplet 3/2 { d8 e r } r2 \bar "|."
} }
% Scripts the brackets avoid, and those they do not.
\score { \relative {
  \tuplet 3/2 { c''4-. d-> e-- } \tuplet 3/2 { f4-^ g\fermata a-1 } |
  \tuplet 3/2 { e,4_. f_> g_- } \tuplet 3/2 { a8-. b-. c-. } r4 |
  \tuplet 3/2 { b4( c d) } \tuplet 3/2 { e4-.( f-. g-.) } \bar "|."
} }
% A fraction, a tuplet slur, a padded number and a bracket with flared
% edges.
\score { \relative {
  \override TupletNumber.text = #tuplet-number::calc-fraction-text
  \tuplet 3/2 { c''4 d e } \tuplet 7/4 { f16 g a b c d e } r4 |
  \revert TupletNumber.text
  \override TupletBracket.tuplet-slur = ##t
  \tuplet 3/2 { c4 b a } \tuplet 3/2 { g8 f e } r4 |
  \revert TupletBracket.tuplet-slur
  \override TupletBracket.bracket-flare = #'(0.5 . 0.5)
  \override TupletNumber.padding = #1
  \tuplet 3/2 { d4 e f } \tuplet 3/2 { g4 a b } \bar "|."
} }
% Two voices, their tuplets up and down.
\score { \new Staff << \relative {
  \voiceOne \tuplet 3/2 { e''4 f g } \tuplet 3/2 { a8 g f } e4 | d2 \tuplet 3/2 { c4 d e } \bar "|."
} \\ \relative {
  \voiceTwo c'4 \tuplet 3/2 { d4 e f } \tuplet 3/2 { g8 f e } | \tuplet 3/2 { d4 c b } a2
} >> }
% A cross-staff tuplet.
\score { \new PianoStaff <<
  \new Staff = "up" \relative { \tuplet 3/2 { c''8 \change Staff = "down" c,, \change Staff = "up" c'' } c2. | c1 \bar "|." }
  \new Staff = "down" { \clef bass s1 | s1 }
>> }
