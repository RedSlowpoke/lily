\version "2.26.0"
% Two scores, translated in the file's order (MS2 step 40): what their
% translations say, the \layout's and the \midi's that the reference
% capture gives a score without one, in the order LilyPond prints it. The
% first \layout's bar check is expected; its \midi's is not. The second
% score's Solo is a context of its \layout only, which its \midi cannot
% create.
#(ly:expect-warning "bar check failed")
\score {
  \new Staff { c'2 c'4 | c'1 }
  \layout { }
}
\score {
  \new Solo { d'4 d'2 | d'1 }
  \layout {
    \context { \Staff \name Solo \alias Staff }
    \context { \Score \accepts Solo }
  }
}
