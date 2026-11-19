\version "2.24.0"
% BPM: 113.76
% Audio: Song (recorded).wav
% Audio offset: -2.40
\header { title = "Lily Import Test" }

global = { \key g \major \time 3/4 \partial 4 }

melody = \relative c'' {
  \global \tempo 4 = 112
  d4 | g2 a4 | b2 a4 | g4 fis e | d2 d4 |
  \tempo 4 = 84
  e2 fis4 | g2 a4 | b4 a fis | g2. \bar "|."
}

\score {
  <<
    \new Staff = "voice" \with { instrumentName = "Voice" midiInstrument = "choir aahs" }
      \new Voice = "v" \melody
    \new Lyrics \lyricsto "v" { \repeat unfold 18 { la } }
    \new Staff = "violin" \with { instrumentName = "Violin" midiInstrument = "violin" }
      \relative c'' { \global b4 | d2. | d2. | b2. | a2 a4 | c2. | b2. | d2 c4 | b2. }
    \new Staff = "cello" \with { instrumentName = "Cello" midiInstrument = "cello" }
      \relative c { \clef bass \global r4 | g2. | g2. | e2. | d2. | c2. | g'2. | d2. | g,2. }
    \new PianoStaff \with { instrumentName = "Piano" } <<
      \new Staff = "rh" \relative c'' { \global r4 | <g b>2. | <g b>2. | <e g>2. | <d fis>2. | <c e>2. | <g' b>2. | <fis a>2. | <g b>2. }
      \new Staff = "lh" \relative c { \clef bass \global r4 | g2. | g2. | e2. | d2. | c2. | g2. | d'2. | g,2. }
    >>
    \new DrumStaff \with { instrumentName = "Drums" } \drummode {
      \time 3/4 \partial 4 hh4 | \repeat unfold 8 { bd4 sn sn }
    }
  >>
  \layout { }
  \midi { }
}
