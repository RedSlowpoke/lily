\version "2.26.0"
% N7's volta iterators where the 30 scores do not reach them (MS2 step
% 37): tail alternatives, \volta in \alternative and in a repeat's body, an
% alternative for every volta, alternatives in the middle of a repeat,
% nested repeats with alternatives, a repeat starting within a bar whose
% alternatives differ in length (alternativeRestores), the alternatives'
% bar numbers, \repeat segno with coda marks and with brackets, and an
% alternative without a volta specification, in two voices.
\new Staff <<
  \relative {
    \set Score.alternativeNumberingStyle = #'numbers-with-letters
    \repeat volta 2 { c'4 d e f } \alternative { { g1 } { a2 b } }
    \repeat volta 3 { c4 d e f }
    \alternative { \volta 1,2 { g1 } \volta 3 { a1 } }
    \repeat volta 2 { c4 \volta 2 d e f } \alternative { \volta 1,2 { g1 } }
    \repeat volta 2 { c4 d \alternative { { e2 } { f2 } } g1 }
    \repeat volta 2 {
      c1
      \repeat volta 2 { d1 } \alternative { { e1 } { f1 } }
    } \alternative { { g1 } { a1 } }
    r2 \repeat volta 2 { c4 d } \alternative { { e2 f4 } { g4 } } a4 b2 |
    \repeat segno 2 { c1 } \alternative { { d1 } { e1 } }
    \repeat segno 2 { c1 \alternative { { d1 } { e1 } } f1 }
    \repeat volta 2 { c1 } \alternative { \volta 1 { d1 } { e1 } }
    c1
  }
  \\
  \relative {
    \repeat volta 2 { e'1 } \alternative { { f1 } { g1 } }
  }
>>
