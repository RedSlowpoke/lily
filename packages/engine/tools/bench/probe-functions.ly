\version "2.26.0"
%% probe-functions.ly: each of the 159 functions music-functions-init.ly
%% defines, applied to small arguments, each call's result shown with
%% \displayMusic (\displayScheme for a Scheme function's, and the state a
%% void function changes for one of those), after a line `=== name` that
%% tools/bench/test/functions.test.mjs splits the output at. LilyPond
%% 2.26.0's output is the reference; the engine's runner gives its own
%% (crates/parser/examples/parse.rs --display). Each result is a
%% variable's value, so the file has no score. Only what the engine's
%% parser reads before P6 is used: no \new, \with, \set or \override, and
%% no variable of the init files that part 2 loads (script-init.ly's
%% articulations, scale-definitions-init.ly's \major, color.scm's colours).

#(define (probe name) (display "\n=== ") (display name) (newline))

#(probe "%")
probeResult = \displayMusic \% #2 c'4
#(probe "*")
probeResult = \displayMusic \* #2 c'4
#(probe "=")
probeResult = \displayMusic c'4 \= #3 -1
#(probe "absolute")
probeResult = \displayMusic \absolute { c'4 d }
#(probe "acciaccatura")
probeResult = \displayMusic \acciaccatura d'8
#(probe "addInstrumentDefinition")
\addInstrumentDefinition "probe" #'((instrumentCueName . "p"))
#(display (assoc-get "probe" instrument-definitions))
#(probe "addQuote")
\addQuote "probe" { c'4 d' }
#(probe "addToTagGroup")
\tagGroup #'(a z)
\addToTagGroup #'(a z) #'(b c)
#(write (tag-group-get 'b))
#(probe "after")
probeResult = \displayMusic \after 8 -1 c'4
#(probe "afterGrace")
probeResult = \displayMusic \afterGrace c'2 { d'16 e' }
#(probe "allowPageTurn")
probeResult = \displayMusic \allowPageTurn
#(probe "alterBroken")
probeResult = \displayMusic \alterBroken #'positions #'((1 . 2) (3 . 4)) #'(Slur)
#(probe "appendToTag")
probeResult = \displayMusic \appendToTag #'t { e'4 } { \tag #'t { c'4 } d'4 }
#(probe "appendToTagMarkup")
probeResult = \displayMusic \appendToTagMarkup #'t "more" { \tag #'t { c'4 } d'4 }
#(probe "applyContext")
probeResult = \displayMusic \applyContext #(lambda (c) c)
#(probe "applyMusic")
probeResult = \displayMusic \applyMusic #(lambda (m) (ly:music-transpose m (ly:make-pitch 0 1 0))) { c'4 }
#(probe "applyOutput")
probeResult = \displayMusic \applyOutput #'(Voice NoteHead) #(lambda (g c o) g)
#(probe "appoggiatura")
probeResult = \displayMusic \appoggiatura d'8
#(probe "approximatePitch")
probeResult = \displayMusic \approximatePitch cis'4
#(probe "assertBeamQuant")
probeResult = \displayMusic \assertBeamQuant #'(0 . 1) #'(0 . 1)
#(probe "assertBeamSlope")
probeResult = \displayMusic \assertBeamSlope #(lambda (l r) #t)
#(probe "atLeft")
probeResult = \displayMusic c'4 \atLeft -1
#(probe "atRight")
probeResult = \displayMusic c'4 \atRight -1
#(probe "autoChange")
probeResult = \displayMusic \autoChange { c'4 d }
#(probe "balloonGrobText")
probeResult = \displayMusic \balloonGrobText #'NoteHead #'(1 . 2) "b"
#(probe "balloonText")
probeResult = \displayMusic c'4 \balloonText #'(1 . 2) "b"
#(probe "bar")
probeResult = \displayMusic \bar "|."
#(probe "barNumberCheck")
probeResult = \displayMusic \barNumberCheck #3
#(probe "beamExceptions")
probeResult = \displayScheme \beamExceptions { c'8[ d'] e'[ f' g' a'] }
#(probe "bendAfter")
probeResult = \displayMusic c'4 \bendAfter #-4
#(probe "bendHold")
probeResult = \displayMusic c'4 \bendHold -1
#(probe "bendStartLevel")
probeResult = \displayMusic c'4 \bendStartLevel #1 -1
#(probe "bookOutputName")
\bookOutputName "probe"
#(probe "bookOutputSuffix")
\bookOutputSuffix "probe"
#(probe "breathe")
probeResult = \displayMusic \breathe
#(probe "caesura")
probeResult = \displayMusic \caesura
#(probe "clef")
probeResult = \displayMusic \clef "bass"
#(probe "codaMark")
probeResult = \displayMusic \codaMark #2
#(probe "compressMMRests")
probeResult = \displayMusic \compressMMRests { R1 R1 }
#(probe "contextPropertyCheck")
probeResult = \displayMusic \contextPropertyCheck #'(Staff instrumentName) "I"
#(probe "crossStaff")
probeResult = \displayMusic \crossStaff { c'4 }
#(probe "cueClef")
probeResult = \displayMusic \cueClef "bass"
#(probe "cueClefUnset")
probeResult = \displayMusic \cueClefUnset
#(probe "cueDuring")
probeResult = \displayMusic \cueDuring "probe" #UP { c'4 }
#(probe "cueDuringWithClef")
probeResult = \displayMusic \cueDuringWithClef "probe" #UP "bass" { c'4 }
#(probe "displayLilyMusic")
probeResult = \displayLilyMusic { c'4 d8 }
#(probe "displayMusic")
probeResult = \displayMusic { c'4 d8 }
#(probe "displayScheme")
probeResult = \displayScheme #'(1 "two" three)
#(probe "dropNote")
probeResult = \displayMusic \dropNote #1 { <c' e' g'>4 }
#(probe "endSpanners")
probeResult = \displayMusic \endSpanners { c'4( d' e' }
#(probe "eventChords")
probeResult = \displayMusic \eventChords { c'4-1 d' }
#(probe "featherDurations")
probeResult = \displayMusic \featherDurations #2 { c'16 d' e' f' }
#(probe "finger")
probeResult = \displayMusic c'4 \finger #3
#(probe "fixed")
probeResult = \displayMusic \fixed c' { c4 e g }
#(probe "footnote")
probeResult = \displayMusic \footnote "*" #'(1 . 2) "note" #'(NoteHead)
#(probe "grace")
probeResult = \displayMusic \grace d'8
#(probe "grobdescriptions")
probeResult = \displayScheme \grobdescriptions #'((NoteHead . ((color . (1 0 0)))))
#(probe "harmonicByFret")
probeResult = \displayMusic \harmonicByFret #12 { c'4\3 }
#(probe "harmonicByRatio")
probeResult = \displayMusic \harmonicByRatio #1/2 { c'4\3 }
#(probe "hide")
probeResult = \displayMusic \hide #'(NoteHead)
#(probe "inStaffSegno")
probeResult = \displayMusic \inStaffSegno
#(probe "initialContextFrom")
probeResult = \displayMusic \initialContextFrom { c'4 }
#(probe "instrumentSwitch")
probeResult = \displayMusic \instrumentSwitch "probe"
#(probe "inversion")
probeResult = \displayMusic \inversion c' e' { c'4 d' }
#(probe "invertChords")
probeResult = \displayMusic \invertChords #1 { <c' e' g'>4 }
#(probe "jump")
probeResult = \displayMusic \jump "D.C."
#(probe "keepWithTag")
probeResult = \displayMusic \keepWithTag #'a { \tag #'a c'4 \tag #'b d'4 }
#(probe "key")
probeResult = \displayMusic \key d #`((0 . 0) (1 . 0) (2 . 0) (3 . 0) (4 . 0) (5 . 0) (6 . 0))
#(probe "killCues")
probeResult = \displayMusic \killCues { c'4 }
#(probe "label")
probeResult = \displayMusic \label #'probe
#(probe "language")
\language "deutsch"
probeResult = \displayMusic { h4 }
\language "nederlands"
#(probe "languageRestore")
\languageSaveAndChange "deutsch"
\languageRestore
probeResult = \displayMusic { b4 }
#(probe "languageSaveAndChange")
\languageSaveAndChange "english"
probeResult = \displayMusic { bf4 }
\languageRestore
#(probe "magnifyMusic")
probeResult = \displayMusic \magnifyMusic #2 { c'4 }
#(probe "magnifyStaff")
probeResult = \displayMusic \magnifyStaff #2
#(probe "makeClusters")
probeResult = \displayMusic \makeClusters { <c' e'>4 }
#(probe "mark")
probeResult = \displayMusic \mark #3
#(probe "markupMap")
probeResult = \displayMusic \markupMap #'text #bold-markup { c'4^"t" }
#(probe "modalInversion")
probeResult = \displayMusic \modalInversion d' e' { c' d' e' f' g' a' b' } { d'4 e' }
#(probe "modalTranspose")
probeResult = \displayMusic \modalTranspose c' d' { c' d' e' f' g' a' b' } { c'4 e' }
#(probe "musicLength")
probeResult = \displayScheme \musicLength { c'4 d'8 }
#(probe "musicMap")
probeResult = \displayMusic \musicMap #(lambda (m) m) { c'4 }
#(probe "noPageBreak")
probeResult = \displayMusic \noPageBreak
#(probe "noPageTurn")
probeResult = \displayMusic \noPageTurn
#(probe "octaveCheck")
probeResult = \displayMusic \octaveCheck c'
#(probe "offset")
probeResult = \displayMusic \offset #'Y-offset #2 #'(NoteHead)
#(probe "omit")
probeResult = \displayMusic \omit #'(NoteHead)
#(probe "once")
probeResult = \displayMusic \once \hide #'(NoteHead)
#(probe "ottava")
probeResult = \displayMusic \ottava #1
#(probe "overrideProperty")
probeResult = \displayMusic \overrideProperty #'(Staff NoteHead color) #'(1 0 0)
#(probe "overrideTimeSignatureSettings")
probeResult = \displayMusic \overrideTimeSignatureSettings 4/4 1/4 3,1 #'()
#(probe "pageBreak")
probeResult = \displayMusic \pageBreak
#(probe "pageTurn")
probeResult = \displayMusic \pageTurn
#(probe "parallelMusic")
\parallelMusic #'(probeA probeB) { c'4 | d'4 | }
probeResult = \displayMusic \probeA
#(probe "parenthesize")
probeResult = \displayMusic \parenthesize c'4
#(probe "partCombine")
probeResult = \displayMusic \partCombine { c'4 } { e'4 }
#(probe "partCombineDown")
probeResult = \displayMusic \partCombineDown { c'4 } { e'4 }
#(probe "partCombineUp")
probeResult = \displayMusic \partCombineUp { c'4 } { e'4 }
#(probe "partial")
probeResult = \displayMusic \partial 4
#(probe "pitchedTrill")
probeResult = \displayMusic \pitchedTrill c'4 -1 d'
#(probe "polymetric")
probeResult = \displayMusic \polymetric \default
#(probe "popContextProperty")
probeResult = \displayMusic \popContextProperty #'(Staff fontSize)
#(probe "preBend")
probeResult = \displayMusic c'4 \preBend -1
#(probe "preBendHold")
probeResult = \displayMusic c'4 \preBendHold -1
#(probe "propertyOverride")
probeResult = \displayMusic \propertyOverride #'(Staff NoteHead color) #'(1 0 0)
#(probe "propertyRevert")
probeResult = \displayMusic \propertyRevert #'(Staff NoteHead color)
#(probe "propertySet")
probeResult = \displayMusic \propertySet #'(Staff fontSize) #2
#(probe "propertyTweak")
probeResult = \displayMusic \propertyTweak #'color #'(1 0 0) c'4
#(probe "propertyUnset")
probeResult = \displayMusic \propertyUnset #'(Staff fontSize)
#(probe "pushContextProperty")
probeResult = \displayMusic \pushContextProperty #'(Staff fontSize)
#(probe "pushToTag")
probeResult = \displayMusic \pushToTag #'t { e'4 } { \tag #'t { c'4 } d'4 }
#(probe "pushToTagMarkup")
probeResult = \displayMusic \pushToTagMarkup #'t "more" { \tag #'t { c'4 } d'4 }
#(probe "quoteDuring")
probeResult = \displayMusic \quoteDuring "probe" { c'4 }
#(probe "raiseNote")
probeResult = \displayMusic \raiseNote #1 { <c' e' g'>4 }
#(probe "reduceChords")
probeResult = \displayMusic \reduceChords { <c' e'>4 <d' f'> }
#(probe "relative")
probeResult = \displayMusic \relative { c'4 g' <c e> f, \octaveCheck c' d
  \key d #`((0 . 0) (1 . 0) (2 . 0) (3 . 1/2) (4 . 0) (5 . 0) (6 . 1/2))
  e c'='4 << { g a } \\ { e f } >> b }
probeResult = \displayMusic \relative c'' { c4
  $(make-music 'NoteEvent 'pitch (ly:make-pitch -1 3) 'duration (ly:make-duration 2)
               'to-relative-callback (lambda (m p) (ly:make-pitch 1 0)))
  d <e g> }
#(probe "removeFromTagGroup")
\tagGroup #'(a z b c)
\removeFromTagGroup #'(a z b c) #'(z)
#(write (list (tag-group-get 'a) (tag-group-get 'z)))
#(probe "removeWithTag")
probeResult = \displayMusic \removeWithTag #'a { \tag #'a c'4 \tag #'b d'4 }
#(probe "replaceWithTag")
probeResult = \displayMusic \replaceWithTag #'t { e'4 } { \tag #'t { c'4 } d'4 }
#(probe "replaceWithTagMarkup")
probeResult = \displayMusic \replaceWithTagMarkup #'t "more" { \tag #'t { c'4 } d'4 }
#(probe "resetRelativeOctave")
probeResult = \displayMusic \resetRelativeOctave c''
#(probe "resetTagGroup")
\tagGroup #'(a b c)
\resetTagGroup #'(a b c)
#(write (tag-group-get 'a))
#(probe "resetTagGroups")
\tagGroup #'(r s)
\resetTagGroups
#(write (tag-group-get 'r))
#(probe "retrograde")
probeResult = \displayMusic \retrograde { c'4 d'8 e' }
#(probe "revertTimeSignatureSettings")
probeResult = \displayMusic \revertTimeSignatureSettings #'(4 . 4)
#(probe "rightHandFinger")
probeResult = \displayMusic c'4 \rightHandFinger #2
#(probe "scaleDurations")
probeResult = \displayMusic \scaleDurations 2/3 { c'8 d' e' }
#(probe "sectionLabel")
probeResult = \displayMusic \sectionLabel "Coda"
#(probe "segnoMark")
probeResult = \displayMusic \segnoMark #2
#(probe "shape")
probeResult = \displayMusic \shape #'((0 . 0) (0 . 1) (0 . 1) (0 . 0)) #'(Slur)
#(probe "shiftDurations")
probeResult = \displayMusic \shiftDurations #1 #1 { c'4 d'8 }
#(probe "single")
probeResult = \displayMusic \single \hide #'(NoteHead) c'4
#(probe "skip")
probeResult = \displayMusic \skip 4
#(probe "slashedGrace")
probeResult = \displayMusic \slashedGrace d'8
#(probe "styledNoteHeads")
probeResult = \displayMusic \styledNoteHeads #'cross #'NoteHead { c'4 }
#(probe "tag")
probeResult = \displayMusic \tag #'a { c'4 }
#(probe "tagGroup")
\tagGroup #'(x y)
#(write (tag-group-get 'y))
#(probe "tagGroupRef")
probeResult = \displayScheme \tagGroupRef #'(u v)
#(probe "temporary")
probeResult = \displayMusic \temporary \propertyOverride #'(NoteHead color) #'(1 0 0)
#(probe "textEndMark")
probeResult = \displayMusic \textEndMark "Fine"
#(probe "textMark")
probeResult = \displayMusic \textMark "Fine"
#(probe "time")
probeResult = \displayMusic \time 3/4
#(probe "timeAbbrev")
probeResult = \displayMusic \timeAbbrev #'(3 . 4)
#(probe "times")
probeResult = \displayMusic \times 2/3 { c'8 d' e' }
#(probe "transpose")
probeResult = \displayMusic \transpose c d { c'4 <e' g'>8
  \key a #`((0 . 0) (1 . 0) (2 . 0) (3 . 1/2) (4 . 0) (5 . 0) (6 . 0)) cis''2 }
#(probe "transposedCueDuring")
probeResult = \displayMusic \transposedCueDuring "probe" #UP c' { c'4 }
#(probe "transposition")
probeResult = \displayMusic \transposition bes
#(probe "tuplet")
probeResult = \displayMusic \tuplet 3/2 { c'8 d' e' }
#(probe "tupletSpan")
probeResult = \displayMusic \tupletSpan 4
#(probe "tweak")
probeResult = \displayMusic \tweak #'color #'(1 0 0) c'4
#(probe "undo")
probeResult = \displayMusic \undo \hide #'(NoteHead)
#(probe "unfoldRepeats")
probeResult = \displayMusic \unfoldRepeats { c'4 }
#(probe "unfolded")
probeResult = \displayMusic \unfolded { c'4 }
#(probe "voices")
probeResult = \displayMusic \voices #'(1 2) << { c'4 } \\ { e'4 } >>
#(probe "void")
\void \displayMusic { c'4 }
#(probe "volta")
probeResult = \displayMusic \volta #'(1) { c'4 }
#(probe "vshape")
probeResult = \displayMusic \vshape #'((0 . 0) (0 . 1) (0 . 1) (0 . 0)) #'(Slur)
#(probe "withMusicProperty")
probeResult = \displayMusic \withMusicProperty #'probe #3 { c'4 }
#(probe "withRelativeDir")
probeResult = \displayScheme \withRelativeDir "probe.ly"
#(probe "end")
