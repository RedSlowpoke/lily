\version "2.26.0"

% MS5 step 3's performers: Tempo_performer's changes, Key_performer,
% Time_signature_performer, Midi_control_change_performer, Staff_performer's
% programs and control initializer, Piano_pedal_performer, and a
% microtone's pitch bend. Two warnings: an out-of-range pan and a pedal
% that never went down.

\score {
  <<
    \new Staff \with {
      midiInstrument = "oboe"
      midiPanPosition = -1.0
      midiReverbLevel = 0.5
    } \relative c'' {
      \tempo 4 = 90
      \key g \major
      g4 a b c
      \set Staff.midiPanPosition = #0.33
      \set Staff.midiBalance = #-0.25
      \tempo "Andante" 4 = 60-70
      d4 cih d deseh
      \time 3/4
      \key e \minor
      \set Staff.midiInstrument = "violin"
      \set Staff.midiExpression = #0.8
      e2 fis4
      \tempo 8 = 132
      \time 6/8
      g8 fis e dis4.
      \set Staff.midiChorusLevel = #1
      \set Staff.midiPanPosition = #2
      \key c \major
      \time 2/2
      c1
    }
    \new Staff \with {
      midiInstrument = "Acoustic Grand"
    } \relative c {
      \clef bass
      \key g \major
      \transposition bes
      c4\sustainOn e g c\sustainOff
      c,4\sostenutoOn e\unaCorda g\treCorde c\sostenutoOff
      \time 3/4
      \key e \minor
      e2.\sustainOn
      \set Staff.midiInstrument = "no such instrument"
      e8 fis g a b4\sustainOff
      \time 2/2
      c1\sostenutoOff
    }
    \new Staff \with {
      midiInstrument = "cello"
      midiExpression = 0.25
      midiChorusLevel = 0.1
    } \relative c {
      \clef bass
      \set Voice.midiReverbLevel = #0.75
      c1
      c4 cisih ceseh c
      \set midiBalance = #1
      c2. c2.
      \tempo \breve = 30
      c1
    }
  >>
  \midi { }
}

% A key and pedals heard in timesteps skipTypesetting skips: the key is
% announced at the next timestep processed, the pedals are dropped.
\score {
  \new Staff \relative c' {
    c4\sustainOn
    \set Score.skipTypesetting = ##t
    \key d \major
    d4\sustainOff e\sustainOn
    \set Score.skipTypesetting = ##f
    f4 g\sustainOff a\sustainOn
    \key f \major
    b4 c\sustainOff
  }
  \midi { }
}
