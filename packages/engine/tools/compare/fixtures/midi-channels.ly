\version "2.26.0"

% MS5 step 5's performer and channels: Drum_note_performer's notes
% (drumPitchTable's pitches, two DrumVoices, a chord, a tie, articulations
% that change a length and a velocity, dynamics, a drum type a table of
% its own lacks), percussion on channel 10, Staff_performer::get_channel's
% 16-channel wrap and its warnings (18 staves), and midiChannelMapping's
% three mappings: staff, voice and instrument (instruments changed in a
% staff, shared by two staves, and a percussion instrument among them).

drh = \drummode { cymc4->\f hh8 hh-. hhc4 hho | <hh sn>4~ q hh16 hh hh8 r4 }
drl = \drummode { bd4\p sn8 bd bd4 <bd ss> | tomh8 tommh toml tomfh bd2\fermata }
line = \relative c' { c4 d8 e f4 g }

\score {
  <<
    \new DrumStaff = "kit" <<
      \new DrumVoice { \voiceOne \drh }
      \new DrumVoice { \voiceTwo \drl }
    >>
    \new DrumStaff = "few" \with {
      drumPitchTable = #(alist->hash-table
                         `((bassdrum . ,(ly:make-pitch -1 0 0))
                           (snare . ,(ly:make-pitch -1 1 SHARP))))
    } \drummode { bd4 sn hh sn | bd2 sn }
    \new Staff = "tune" \with { midiInstrument = "marimba" } \line
  >>
  \midi { \tempo 4 = 96 }
}

\score {
  <<
    \new Staff = "s1" \line
    \new Staff = "s2" \line
    \new Staff = "s3" \line
    \new Staff = "s4" \line
    \new Staff = "s5" \line
    \new Staff = "s6" \line
    \new Staff = "s7" \line
    \new Staff = "s8" \line
    \new Staff = "s9" \line
    \new Staff = "s10" \line
    \new Staff = "s11" \line
    \new Staff = "s12" \line
    \new Staff = "s13" \line
    \new Staff = "s14" \line
    \new Staff = "s15" \line
    \new Staff = "s16" \line
    \new Staff = "s17" \line
    \new DrumStaff = "s18" \drummode { bd4 sn bd sn }
  >>
  \midi { }
}

\score {
  <<
    \new Staff = "upper" <<
      \new Voice = "one" { \voiceOne \line }
      \new Voice = "two" { \voiceTwo \transpose c a, \line }
    >>
    \new Staff = "lower" \with { midiInstrument = "cello" } <<
      \new Voice = "three" { \clef bass \transpose c c, \line }
    >>
    \new DrumStaff = "drums" \drummode { bd4 sn bd sn }
  >>
  \midi {
    \context { \Score midiChannelMapping = #'voice }
  }
}

\score {
  <<
    \new Staff = "first" {
      \set Staff.midiInstrument = "violin"
      \line
      \set Staff.midiInstrument = "flute"
      \line
      \set Staff.midiInstrument = "drums"
      \line
      \set Staff.midiInstrument = "violin"
      \line
    }
    \new Staff = "second" \with { midiInstrument = "flute" } <<
      \new Voice = "a" { \voiceOne \line \line \line \line }
      \new Voice = "b" { \voiceTwo \transpose c g, { \line \line \line \line } }
    >>
    \new Staff = "third" \with { midiInstrument = "oboe" } { \line \line \line \line }
  >>
  \midi {
    \context { \Score midiChannelMapping = #'instrument }
  }
}
