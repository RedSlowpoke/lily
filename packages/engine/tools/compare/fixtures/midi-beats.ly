\version "2.26.0"

% Beat_performer (scheme-performers.scm, Scheme in the image, MS5 step
% 5): an accent on each beat of beatStructure and a marcato at each bar's
% start through its articulation events, beatExtraVelocity and
% barExtraVelocity changed, an accent off the beat silencing the next
% beat's, a cadenza without timing, and a DrumVoice, which
% Drum_note_performer plays.

\score {
  <<
    \new Staff \with { \consists Beat_performer } \relative c'' {
      \time 4/4
      c8 c c c c c c c |
      c8 c c-> c c c c4 |
      \time 6/8
      c8 c c c c c |
      \set Staff.beatExtraVelocity = 30
      \set Staff.barExtraVelocity = 0
      c4. c8 c-^ c |
      \time 7/8
      \set Timing.beatStructure = #'(2 2 3)
      c8 c c c c c c |
      \cadenzaOn
      c8 c c c
      \cadenzaOff
      \bar "|"
      c8 c c c c c c |
    }
    \new DrumStaff \drummode {
      \new DrumVoice \with { \consists Beat_performer } {
        sn8 sn sn sn sn sn sn sn |
        sn8 sn sn-> sn sn sn sn4 |
        sn8 sn sn sn sn sn |
        sn4. sn8 sn sn |
      }
    }
  >>
  \midi { \tempo 4 = 80 }
}
