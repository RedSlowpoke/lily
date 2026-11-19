\version "2.26.0"
% A bar number whose text is '() (MS3 step 40; step 18's Open): with no
% barNumberFormatter procedure Bar_number_engraver gives its BarNumber the
% text '(), whose print gives LilyPond's programming error "Trying to
% interpret a non-markup object: ()" each time it is read. Every bar number
% is visible, so that the three are live on the one line, placed by
% self-alignment-interface::self-aligned-on-breakable (break-alignment-
% interface.cc's self_align_callback) and side-position-interface.cc; the
% one staff's SystemStartBar collapses and is killed by its print.
\layout {
  \context {
    \Score
    barNumberVisibility = #all-bar-numbers-visible
    barNumberFormatter = ##f
    \override BarNumber.break-visibility = #all-visible
  }
}
{ c'1 c'1 }
