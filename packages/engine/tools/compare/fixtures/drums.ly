\version "2.26.0"
% Drum_notes_engraver (MS4 step 13, for step 14): a DrumStaff in
% \drummode, the head styles of drums-style and their scripts, two voices,
% the other styles' staves (timbales, congas, bongos, percussion), a
% drumStyleTable of its own with a script's direction, and ties, slurs, a
% tuplet and beams on drum notes.
% The drums of drums-style: styles, positions and scripts.
\score { \new DrumStaff \drummode {
  bd4 sn hh cymc | hho hhc hhho hhp | tomh tommh tomml toml |
  tomfh tomfl cymr cymch | cb wbh wbl trio | sna ss sne ssh |
  hhho8 hho hhc hh cyms cymca rb cymr \bar "|."
} }
% Two voices, up and down, with rests and a tuplet.
\score { \new DrumStaff <<
  \new DrumVoice \drummode { \voiceOne hh8 hh hh hh \tuplet 3/2 { hh8 hho hhc } hh4 | cymc4 hh8 hh r2 \bar "|." }
  \new DrumVoice \drummode { \voiceTwo bd4 sn bd8 bd sn4 | bd4 r sn8 sn sn4 }
>> }
% Timbales, congas, bongos and percussion, each with its staff and style.
\score { <<
  \new DrumStaff \with { drumStyleTable = #timbales-style \override StaffSymbol.line-count = #2 }
    \drummode { timh4 ssh timl8 ssh timl timh | cb4 timl r2 \bar "|." }
  \new DrumStaff \with { drumStyleTable = #congas-style \override StaffSymbol.line-count = #2 }
    \drummode { cgh4 cgho cghm ssh | cgl cglo cglm r \bar "|." }
  \new DrumStaff \with { drumStyleTable = #bongos-style \override StaffSymbol.line-count = #2 }
    \drummode { boh4 boho bohm ssh | bol bolo bolm r \bar "|." }
  \new DrumStaff \with { drumStyleTable = #percussion-style \override StaffSymbol.line-count = #1 }
    \drummode { tri4 trio trim gui | guis guil cl cab \bar "|." }
>> }
% A drumStyleTable of its own: a script with its direction, one without a
% style, a drum the table lacks.
#(define mydrums '((bassdrum default (accent . -1) -3)
                   (snaredrum xcircle (staccato . 1) 1)
                   (hihat cross #f 5)
                   (lowtom () tenuto -1)))
\score { \new DrumStaff \with { drumStyleTable = #(alist->hash-table mydrums) }
  \drummode { bd4 sn hh toml | bd8 sn hh toml cymc4 bd \bar "|." }
}
% Ties, slurs and beams on drum notes.
\score { \new DrumStaff \drummode {
  sn2~ sn | bd8( sn bd sn) hh[ hh hh hh] | <bd sn>2~ <bd sn>4 hh \bar "|."
} }
