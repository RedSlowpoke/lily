\version "2.26.0"
% Page markers and labels (MS3 step 64): toplevel \label, \pageBreak and
% \noPageBreak between the scores and markups (page markers, which set the
% permission of what is before them and label what is after them), a
% \label in the music, and \page-ref and \with-link that read the labels'
% pages once the pages are made; the pages numbered in roman-lower (B7).
\paper {
  page-number-type = #'roman-lower
  print-first-page-number = ##t
}
\markup { See page \page-ref #'second "0" "?" for the second score. }
\label #'first
\relative { c''4 d e f \label #'inside | g1 \bar "|." }
\markup \with-link #'second { A link to the second page }
\pageBreak
\label #'second
\markup { The first score is on page \page-ref #'first "0" "?", its label inside on \page-ref #'inside "0" "?". }
\noPageBreak
\relative { e'4 f g a | b1 \bar "|." }
