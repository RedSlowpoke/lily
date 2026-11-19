# LilyPond ecosystem prior art for Lily's visual editor

Nobody outside Spontini and Denemo writes edits made on the rendered score back into `.ly` text, and both of those are GPL. But LilyPond 2.26 can already give Lily everything it needs per grob: the source span, the moment, the voice and staff, pitch, duration, and even *which use of a variable* a note comes from. I tested that on the installed 2.26.0. The concrete design is in "How Lily should get per-grob metadata" near the end.

The test scores are in [exp/](exp/); `big.ly`, the 4 × 120-bar score used for the cost, is not kept. Two subagents covered Spontini/Denemo and the rest of the ecosystem; I read Frescobaldi, python-ly and LilyPond myself.

**[V]** = read in source/docs or tested. **[I]** = inferred. **[A]** = read in source by a subagent, not re-checked by me.

---

## 1. python-ly
https://github.com/frescobaldi/python-ly, commit 886ea0d. Licence: GPL-3.0-or-later (`REUSE.toml`).

- **Editing preserves formatting because it only ever replaces token spans [V].**
  - `ly/document.py` `DocumentBase.__setitem__` queues `(start, end, text)` changes. `__exit__` sorts them in reverse, `check_changes` rejects overlaps, and `update_cursors` shifts live cursors.
  - `Document.apply_changes` re-lexes only from the changed line, and stops once a line's frozen lexer state matches the old one.
  - Nothing is ever pretty-printed back.
- **Pitches [V].** `ly/pitch/__init__.py`:
  - `PitchIterator.pitches()` groups the Note, Octave, Accidental and OctaveCheck tokens into a `Pitch`. It follows `\language` and `\include "xx.ly"`.
  - `write()` rewrites only the tokens that changed.
  - The `\relative` arithmetic is `makeAbsolute`/`makeRelative`: `octave += last.octave - (note - last.note + 3)//7`.
- **`\relative` [V].** `ly/pitch/transpose.py` `transpose()` → `relative()`:
  - It walks the tokens with a `lastPitch`. Inside a chord the reference is the chord's first note (`lastPitch = chord[:2][-1]`).
  - It honours `\octaveCheck`, skips `\transposition` and `\stringTuning`, and handles `\key` and chordmode specially.
  - **Key point:** after changing a pitch, `transposeRelative` makes the next note relative to the *changed* pitch. Editing one note under `\relative` can force octave marks to be rewritten on the note after it.
  - There is no `\fixed` support (grep finds nothing).
  - It cannot know what a variable used inside `\relative` resolves to.
- **Durations [V].** `ly/rhythm.py`:
  - `music_items()` gives each note/rest/skip its token span, its duration tokens and an `insert_pos`.
  - `preceding_duration()` scans backwards for the inherited duration.
  - `rhythm_implicit`/`explicit` add or remove repeated durations.
  - So changing or deleting a note whose duration is implicit means making the next note's duration explicit.
- **`ly.music` tree [V].** `ly/music/items.py`:
  - It is a tree over the tokens: Relative, Transpose, Repeat, Chord, `UserCommand.value()` (looks up the variable's assignment, following includes), and so on.
  - `Document.node(pos)` bisects to the item at a position. `time_position(pos)` and `time_length` compute musical time from the *text*.
  - It is a heuristic interpreter: no Scheme, no real music-function arity.
- **Reuse:** GPL, so no code in an MIT app. The algorithms are LilyPond semantics and can be reimplemented from the LilyPond docs.

## 2. Frescobaldi
https://github.com/frescobaldi/frescobaldi, commit 58cf8c8. GPL-2.0-or-later headers, GPL in the README.

- **Point-and-click [V].** `pointandclick.py`:
  - `Links` groups links by file. `BoundLinks` turns every `textedit` link into a `QTextCursor` in the open document, so **links follow the user's edits until the next compile**.
  - Lily's D19 says this is "not compensated". Monaco decorations would do it.
  - `indices()` also finds the link of a slur or beam when the cursor is on its *closing* token, by scanning back to the matching opening token.
- **Edit in Place [V].** `editinplace.py`; triggered by Shift+click in `musicview/widget.py` `slotLinkClicked`:
  - A popup edits the clicked note's whole source line, from its indentation to the end of the line.
  - Saving goes through `cursordiff.insert_text`, which applies a `difflib` diff of old and new text rather than one replacement, explicitly "so point and click positions are not destroyed".
  - It is text editing at the clicked place, not music editing.
- **Quick Insert [V].** `quickinsert/articulations.py`, `spanners.py`:
  - These work on the *text cursor or selection*: `articulation_positions` → `ly.rhythm.music_items` → insert after `item.end`.
  - A click in the music view only moves the cursor. Selecting in the view (the rubberband) copies an image or text; it does not select music.
- **Transpose, rhythm, rel↔abs:** python-ly applied to the selection [V].
- **Dragging in the view: experimental and barely built [V].**
  - `svgview/editsvg.js` makes every `<a>` that has a `transform` draggable and reports the `textedit` href and offsets.
  - `objecteditor/defineoffset.py` maps the clicked `ly.music` item class to a grob: only String/Markup → TextScript, Tempo → Score.MetronomeMark, Articulation → Script, and otherwise `"still testing!"`.
  - It then inserts `\once \override X.extra-offset = #'(x . y)` on a new line *before the line* holding the item, using spin-box values.
  - The panel loads only from git or with "experimental features" (`panelmanager.py:84`).
  - Lesson: a source token and its item class are not enough to know the grob and its context.

## 3. Spontini-Editor
https://github.com/paopre/Spontini, by Paolo **Prete** (not Sorrentino), v1.31_alfa, last commit 2025-07. Licence: GPLv3+. [A]

- **Architecture:** a Python server runs LilyPond's SVG backend; a CodeMirror page in the browser. Its README says it is "NOT a WYSIWYG notation editor": there is no pitch or duration editing.
- **Identity through explicit ids in the source:**
  - The user inserts wrappers such as `c' \jsTweak "1" "extra-offset" #'(3.868 . -4.322) \mf`, or `\jsShape "22" "cpts" …`, `\jsOnceOverride`, `\jsOnceOffset … positions`.
  - `lib/ly/jssvg.ly` `createJSSVGEditableObj` turns each into the native `\tweak`/`\shape`/`\once \override`. It also stamps `output-attributes` (`spontinieditable`, the id, `editcommand`) and `staffspace` from `ly:staff-symbol-staff-space`.
  - `\jsShape` replaces the stencil so that the control points are drawn as circles the JavaScript can read. Padding properties get reference-line markers.
- **Drag → text** (`svg-and-pdf-functions.js`, `cm-wrap.js`):
  - It reads the existing value from the text by regex.
  - origin = current SVG position − value × staffspace; new value = (new − origin) / staffspace, with y inverted.
  - The wrapper is found again by id and property, and its string replaced.
  - Textedit positions are shifted by the edit delta between compiles (`__updatePointAndClickPositions`).
- **Weak points:**
  - Two steps: insert the wrapper, recompile, then drag.
  - The source is cluttered with ids, and the file depends on Spontini's include.
  - Values are parsed by regex.
  - Version fudge factors for 2.22 vs 2.23 and NoteColumn.
  - A variable used twice shares one id, so both instances move together.
- **Reuse:** ideas only, being GPL.

## 4. Denemo
git.savannah.gnu.org/git/denemo.git. Licence: GPLv3+. [A]

- **The model is primary.** `DenemoProject` → `DenemoMovement` → staff → measure → `DenemoObject` (chord/notes with `mid_c_offset`, and so on). It saves its own XML. LilyPond is regenerated in full by `exportlilypond.c`, with **absolute pitches only**.
- **Hand-written LilyPond survives only as `DenemoDirective`s** (prefix/postfix text attached to objects).
  - The LilyPond View is generated text. Only the directive "islands" between `GtkTextChildAnchor` pairs are editable; `merge_lily_strings` writes them back into the model.
  - An edit is refused if the model changed since the text was generated.
  - Undo is disabled there.
  - A layout "reduced to LilyPond" can no longer be edited graphically.
- **Click → model object** (`printview.c` `goto_lilypond_position`): find the line/column in the generated buffer, then the nearest preceding navigation anchor. This is exact only because Denemo wrote the text itself.
- **Drag tweaks** (`wysiwyg.scm`): an `extra-offset` override that accumulates, or absolute `control-points`. The pixel-to-staff-space factor is "trial and error". Curves need four clicks.
- **Import of `.ly`** is a 2010 parser port that turns the parse into editor commands and drops overrides, `\set` and contexts. It is lossy and permanently behind the grammar; users report failures and crashes.
  - `docs/DESIGN.lilypond` records an abandoned text-first design that kept each item's original text. It stalled on "where does a new definition go", ties, duration/octave context at bar boundaries, and incomplete grammar rules.
- **Playback view** (`actions/lilypond/2.26/live-score.ily`): a NoteHead `output-attributes` grob-transformer writes `id="Note-LINE-COL"`, and an event-listener file gives moments joined by that id. The same idea as Lily's D26.
- **Lesson:** model-first gives solid note editing but makes hand-written LilyPond second-class, and import is a moving target. Text-first, as Lily is, avoids that but has to solve the mapping in reverse. Neither project handles a variable instantiated twice, re-octaving under `\relative` after an edit, or music produced by functions.

## 5. LilyPond itself: what it can tell us
All of this was tested on the installed 2.26.0; the files are in [exp/](exp/).

- **`textedit` links [V]** (`scm/lily/output-svg.scm` `grob-cause`):
  - The link is emitted only when the grob's `cause` is a stream event with an `origin`. It gives start line, char and column+1 only.
  - `-dpoint-and-click` also accepts an event class or a list of classes.
- **The grob → event chain, from a Score-level `acknowledgers ((grob-interface engraver grob source-engraver))` [V]** (`dump.ly`):
  - These grobs have the event as their `cause`: NoteHead, Rest, Script, Slur, DynamicText, KeySignature (`\key`) and TimeSignature (`\time`).
  - These have another grob as their cause: Stem, Flag, NoteColumn and the automatic Beam (cause chain e.g. Beam → Stem → NoteHead).
  - Clef, BarLine and StaffSymbol have none.
  - `(ly:translator-context source-engraver)` gives the creating context (`Voice/v1`, `Staff/up`). `ly:context-current-moment`, `currentBarNumber` and `measurePosition` are available at that point.
  - In an unfolded repeat, both copies of a note have the same origin but different moments.
- **Source spans [V]** (`span.ly`):
  - `ly:input-both-locations` gives start *and end*, but in **tab-expanded columns, not characters**. With a leading tab, the start char is 13 while the column is 20.
  - A note event's span runs from the pitch through its post-events (`c4-.` is 22–26, `d(` is 27–29, `f\p` is 38–41). Each post-event has its own span (`-.` is 24–26).
  - In a chord, each note's span is just its pitch (`<e g>8` gives 31–32 and 33–34), while its event still has duration 8.
  - The end can be on a later line.
- **`output-attributes` [V]** (`meta.ly`, `broken.ly`):
  - Setting it in that engraver produces `<g id="g3" data-grob="NoteHead" data-ctx=… data-at="0" data-pitch="1 0 0" data-dur="4" data-span="(41 22 41 26)">`, wrapping `<a xlink:href="textedit:…">`.
  - It also works as a **callback** (`\override X.output-attributes = #proc`). The callback is evaluated **once per broken piece**: a slur over a line break gave two distinct ids. A 2020 Kastrup post saying it can't take a callback is out of date.
  - Lily's sanitiser (`web/preview.js` `allowedAttribute`) keeps `id` and `data-*`.
  - [I, not tested] An alist set at acknowledge time would be copied unchanged to broken pieces, giving duplicate ids. That is why the recommendation below uses a closure.
- **Variable use sites are recoverable [V]** (`tree.ly`, `link2.ly`):
  - `toplevel-music-functions` (`scm/lily/music-functions.scm:1608`) runs on `\score` music too, and works when loaded through `-dinclude-settings`.
  - A walk there can set a custom music property such as `lily-chain` on every event: the list of enclosing expressions with their spans. Deep copies carry it, including the copies `\repeat unfold` makes while iterating (`make-unfolded-set`), so the engraver reads it back with `ly:event-property ev 'lily-chain`.
  - Matching music objects by identity (`hashq`) fails for those copies; the property does not.
  - The copy of a referenced variable carries the **use site's** location: `\mel` at 20:22–26, `\plain` at 19:23–29.
  - `\relative` (RelativeOctaveMusic) and `\transpose` (TransposedMusic) wrappers show up in the chain.
- **Pitch pitfalls [V]:**
  - Under `\transpose c d`, the event's pitch is `fis` while the text says `e`.
  - An absolute variable referenced inside `\relative` gets relativised: `{ g'4 a' }` came out as g'' a'''.
  - So the editor must edit by *delta from the written token*, never by writing the event's absolute pitch.
- **Clefs [V]:** `\clef` creates no event, and its SetProperty origins are dummies (`0 0 0 0`). An `ApplyContext` placed after each clef `ContextSpeccedMusic` by a `music-map` in the walk reported `clefs.G at 1 … (22 24 22 36)`.
- **Units [V]:** the viewBox is 119.5 units across 210 mm, so 1 SVG user unit = 1 staff space at the global staff size. Staves with a different size need their own staff-space (Spontini's lesson). SVG y is LilyPond's −y.
- **`\displayLilyMusic` [V]** turned `\relative { c''4-. <e g>8( f) … }` into `\absolute { c''4-. < e'' g'' >8( f''8) … }`. It canonicalises and loses relative mode, implicit durations and comments, so it is only usable for generating new snippets.
- **`-dsvg-woff`** does not exist in 2.26 (absent from `-dhelp`) [V].
- **Cost [V]** (`big.ly`, a 4 × 120-bar test score): compile time within noise (42.25/41.21 s without, 42.78/42.17 s with). SVG +44% raw (2.94 → 4.22 MB over 6 pages); page 1 gzipped went from 19 to 40 KB.

## 6. Other projects
Mostly [A]; the subagent read their source.

- **hacklily** (AGPL):
  - It uses Cairo SVG, which has no links, so it ships a patched libcairo to add them.
  - It misreads the three link numbers as line/start/end; they are line, char and column+1.
  - Its "Makelily" palette only inserts newly generated snippets at the cursor.
  - Lesson: stay on the Scheme SVG backend. Cairo has neither links nor `output-attributes`.
- **ly-lsp + lilypond-studio** (orac, **MIT**, Rust, active 2026): the closest prior art for Lily's text side.
  - It is built on tree-sitter. `src/note_analyser.rs` `analyse()` resolves `\relative` octaves (`relative_octave`), inherited durations, the note language and chords into events with spans.
  - Command arity comes from signatures read from the installed LilyPond's `ly/*-init.ly` and the user's own `define-music-function`s.
  - Code actions make real source rewrites: extract/inline a variable, unfold repeats, make pitches and durations explicit.
  - It lists its known limitations at the top of the file.
- **Swara's tree-sitter-lilypond** (MIT, Oct 2026, young, one author): real `note`/`pitch`/`duration`/`chord` nodes and post-event nodes; validated against 2.26.0.
- **nwhetsell/tree-sitter-lilypond** (MIT): essentially a flat token-and-bracket layer, by design, because music-function arity "is ambiguous".
- **Canorus** (GPL): model-first. Its source view re-imports and *replaces the whole voice*.
- **Elysium** (EPL): a permissive Xtext grammar for navigation only.
- **Abjad**: a faithful PLY port of `parser.yy`, but output is normalised. Its licence is ambiguous (pyproject says MIT, the LICENSE file is GPL-3).
- **Rosegarden, MuseScore, NoteEdit, Laborejo**: export only. MuseScore dropped its LilyPond import.
- **edition-engraver** (openlilylib, GPL): tweaks applied from *outside* the music, addressed by context id, bar and moment.
- **Verovio**: an element id in the SVG equals the model id. This is the clean design LilyPond lacks.
- **Mailing lists:**
  - Kastrup (lilypond-devel 2026-03): an outside parser should generate its rules from the signatures of the installed LilyPond.
  - Paolo Prete's 2019 curve-dragging thread, which became Spontini: Urs Liska warned that a new `\shape` stacked on an old one gives unexpected results, so replace the existing one; prefer absolute values and stable ids.
  - Why WYSIWYG efforts stall: one token yields many grobs, engraver-made grobs have no token, arity needs Scheme evaluation, layout is global, and the text has to stay idiomatic.

---

## Lessons for Lily, ranked

1. **Get identity from LilyPond, not from guessing.** Use an injected engraver plus a music-tree walk (design below). An href alone cannot tell apart two uses of a variable, the two passes of an unfolded repeat, or which grob type was hit.
2. **Write edits as minimal span edits on tokens; never reprint music.** python-ly's change list and Frescobaldi's `cursordiff` both do this, and `\displayLilyMusic` shows what reprinting does to the text. In practice that means four things:
   - The spans come from an MIT parser of our own. Base it on ly-lsp (Rust, MIT, with signatures from the installed `ly/`) using nwhetsell's or Swara's grammar, and cross-check it against LilyPond's `data-span`.
   - Map positions through edits made since the compile, as Frescobaldi's bound cursors and Spontini's delta shift do: Monaco tracked decorations.
   - Check that the span's text is still what was compiled before applying an edit.
   - Convert columns to characters.
3. **Pitch edits are deltas against the written token.**
   - Read the written pitch from the token.
   - Re-spell it under `\relative` using the previous note's *absolute* pitch from LilyPond's map. That avoids simulating `\relative`, variables inside it included [I].
   - Then fix the next note's octave marks so that it keeps its pitch, which is python-ly's `transposeRelative` lesson. In a chord the reference is its first note. `\transpose` and relativised variables mean the event's pitch ≠ the written pitch.
4. **Duration edits and deletes must make the next note's duration explicit** when it was inherited (python-ly `preceding_duration`). Insertions take `insert_pos` after the duration and post-events.
5. **One span, many grobs is a UI decision, not a bug.** A variable or unfolded repeat: show every place (D19 already marks them all) and say "this edit changes N places". Offer to copy or unfold first, which is ly-lsp's code action. For per-instance tweaks, address by context, bar and moment (the edition-engraver idea) or wrap that use in `\tweak`. Spontini's shared id moves both instances.
6. **The cheapest first deliverable, Frescobaldi-style:** a selection in the score becomes a text range or a set of event spans, and Lily runs text transforms on it (articulation, dynamics, slur, transpose, rhythm). Edit in Place is just Monaco focused on the span.
7. **Drag tweaks: units and replacement.**
   - 1 SVG unit = 1 global staff space; divide by the grob's own staff space; invert y.
   - Find and *replace* an existing `\tweak`, `\offset` or `\shape` at the target instead of stacking one (Liska). The value = (new − (current − old value)).
   - Read slur `control-points` through `output-attributes` rather than drawing them into the stencil.
   - Defer this until the other edits exist; every predecessor got stuck here.
8. **Keep the Scheme SVG backend** (D1). Cairo has neither links nor `output-attributes`.
9. **Do not go model-first.** Denemo, Canorus and MuseScore show import is lossy and permanently behind LilyPond's grammar.

## How Lily should get per-grob metadata (recommendation)

All three pieces below go in one runtime file passed with `-dinclude-settings`, alongside `timing.ly` (D26). I have not tested passing two `-dinclude-settings`, so assume a single file.

**A. The music-tree walk.** Append it to `toplevel-music-functions`; this works through `-dinclude-settings` [V].
- On every event it sets a `lily-chain` music property: `((name file l1 c1 l2 c2) …)` for each enclosing expression with an origin. That gives:
  - **variable use sites:** a chain entry whose span covers a `\name` token;
  - `RelativeOctaveMusic`, `TransposedMusic` and repeats.
- It also adds an `ApplyContext` probe after each `\clef`, which yields the clef's origin, moment and context [V].

**B. A Score-level engraver.** It acknowledges `grob-interface` and, for each grob, records:
- `grob::name`;
- the event found by following `cause` through grobs, with:
  - event class
  - pitch (octave, notename, alteration)
  - duration
  - `ly:input-both-locations` plus `ly:input-file-line-char-column` (the start char), and the file
  - `lily-chain`
- the context path. Use `ly:context-id` where it is set, otherwise a synthetic `Voice#n` in the order contexts are first seen;
- the moment (main and grace), `currentBarNumber` and `measurePosition`.

It sets `output-attributes` to a closure that writes `data-lily-id="N"`, or `"N.k"` for broken piece *k*. At `finalize` it writes `<output>.score-map.json`, keyed by id, as `timing.ly` does. For spanners the closure adds both bounds' column `when`.

Ids plus a side file keep the SVG growth small. Putting all the data in `data-*` attributes is the simpler alternative; I measured that at +44% raw SVG.

**C. On the host side:**
- Convert columns to characters against the snapshot's lines.
- Map through `SourceSnapshot.links` (D25).
- Treat the map as untrusted: a score can set `output-attributes` itself.
- The textedit hrefs stay exactly as they are (D19 and D26 are unaffected).

**Not yet verified:**
- grace-note moments;
- lyrics, chord names and figures;
- `\partcombine` and cross-staff notes;
- music made by user Scheme without an origin;
- `\fixed`;
- whether `TransposedMusic` stores its interval (I recommend deriving it from the token anyway);
- `\book`/`\bookpart`;
- interaction with the D25 warm worker (each request forks, so the globals should be per request, but this is untested).

## Licences and what can be reused

| Project | Licence | Reusable in Lily (MIT) |
|---|---|---|
| python-ly, Frescobaldi | GPL-3+ / GPL-2+ | Ideas and algorithms only |
| Spontini, Denemo, Canorus, edition-engraver, Rosegarden | GPL-3+ | Ideas only |
| hacklily | AGPL-3+ | Ideas only |
| Elysium | EPL-1.0 | Ideas only |
| Abjad | Ambiguous (MIT in pyproject, GPL-3 LICENSE) | Treat as GPL |
| ly-lsp, Swara grammar, nwhetsell grammar, LilyBin | MIT | Usable as code |
| Lily's own runtime `.ly` | — | Already runs inside GPL LilyPond, as `timing.ly` does; it uses only public API |

Denemo's `live-score.ily` is derived from LilyPond's GPL `event-listener.ly`; write ours from scratch.

