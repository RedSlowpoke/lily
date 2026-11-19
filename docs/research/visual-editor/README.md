# Editing on the score: findings and plan

Whether Lily can let the user edit music by acting on the rendered score, as
well as by typing, and how. The `.ly` text stays the source of truth. The
score becomes a second way to make edits to that text, and both views stay
side by side. Three surveys of other applications were made for it:

| Chapter | Subject |
| --- | --- |
| [01](01-lilypond-ecosystem.md) | LilyPond's own ecosystem, from the source: python-ly, Frescobaldi, Spontini-Editor, Denemo, and what LilyPond 2.26 can tell us about a grob |
| [02](02-notation-formats.md) | Other notation text formats with visual editing (mei-friend, Verovio Humdrum Viewer, Neon, Verovio's editor toolkit, abcjs, alphaTab, Strudel) and the note input of MuseScore, Dorico, Sibelius and Finale |
| [03](03-general-bidirectional.md) | Text ↔ visual editing outside music: SyncTeX, Overleaf's Visual Editor, LyX, Typst, Markdown live preview, SwiftUI and Flutter canvases, Sketch-n-Sketch, projectional editors, lossless syntax trees |

Each chapter marks what was read in source or docs and what is inferred.
§4's spike and chapter 01's experiments were run on LilyPond 2.26.0. Their
scores are in [exp/](exp/), and `spike-edit-map.ly` is §4's engraver.

## 1. The answer: possible, within a stated scope

Yes. Lily already has most of what an editor on the score needs, and
LilyPond 2.26 can report the rest without being patched. Nothing has to be
built that has never been built: every part below has a working precedent in
another application. What is not possible is a MuseScore that happens to save
`.ly`. A score is a program: variables, music functions, Scheme, includes.
Music that cannot be traced to a note written in the text cannot be edited on
the score. A click on it leads to the code instead, as it does today.

The scope that works is that of Verovio Humdrum Viewer (02 §2), the closest
working analogue:

- Select notes on the score.
- Change their pitch, accidental, octave and duration.
- Turn notes into rests and back, and delete them.
- Add and remove articulations, dynamics, slurs and ties.
- Enter new notes from a caret with the keyboard.

Every one of these is a small replacement in the text, under the user's own
formatting, and one ⌘Z undoes it. Moving things by dragging them is a later,
separate phase (§5, P6). Lyrics, restructuring rhythms, and anything layout
does are left to the text.

## 2. How others do it

Three architectures, and what became of them:

| Architecture | Examples | Outcome |
| --- | --- | --- |
| **The model owns the music; the text is generated from it** | Denemo, LyX, TeXmacs, Neon, Verovio's editor toolkit, alphaTab, Typora | Solid note editing, but text the user wrote by hand becomes second-class. Denemo keeps it only as opaque "directives" and its `.ly` import is permanently behind the grammar (01 §4). Users complain about reformatting (03 §1, §3). GitLab gave up preserving the original markdown, calling it "uncapped R&D" (03). |
| **The text owns the music; an edit on the view patches a span of text** | Verovio Humdrum Viewer, abcjs, mei-friend, Overleaf's Visual Editor, python-ly/Frescobaldi, Spontini-Editor | Formatting, comments and abstractions survive, because only the edited tokens change. Overleaf's rules (03 §1): "perform minimal edits; write appropriate LaTeX; if we don't understand it, we don't edit it". |
| **Projectional: no text at all** | JetBrains MPS, Hazel | High cost in usability (03 §5). Not for a text format people already write. |

Lily is a text-first tool, so it takes the second. Two of its failures are
worth avoiding:

- mei-friend needed four undos for one pitch change (02 §1.4).
- Frescobaldi's drag panel stopped at "still testing!", because a source
  token does not say which grob or context it made (01 §2).

Nobody outside Denemo and Spontini (both GPL) writes edits made on a
LilyPond score back into text. Spontini edits only layout tweaks, and only
through ids the user first inserts by hand (01 §3).

No code is worth importing:

- python-ly, Frescobaldi, Spontini and Denemo are GPL.
- mei-friend, verovio-editor and Strudel are AGPL. Verovio Humdrum Viewer has
  no licence.
- abcjs, Neon, VexFlow and OSMD are MIT or BSD, but offer patterns rather
  than parts.

python-ly's algorithms (`\relative` arithmetic, implicit durations) are
LilyPond's semantics, and are reimplemented from LilyPond's documentation.

Two MIT projects are close enough to Lily's text side to be considered as
code (01 §6):

- **ly-lsp** (Rust, built on tree-sitter, active in 2026). Its
  `note_analyser.rs` resolves `\relative`, inherited durations, the note
  language and chords into events with spans. It reads the arity of music
  functions from the installed LilyPond's `ly/*-init.ly`, and of the score's
  own `define-music-function`s.
- **Swara's tree-sitter-lilypond** (October 2026, young). It has real
  `note`, `pitch`, `duration`, `chord` and post-event nodes, validated
  against 2.26.0.

## 3. What Lily has already

- **Every notehead, rest, script, dynamic and slur in the preview is a link
  to its source** (`textedit://FILE:LINE:CHAR:COL`, D1, D19). Studio parses
  it (`parseTextEdit`) and opens the place (D32).
- **A Scheme performer injected into every preview compile** with
  `-dinclude-settings` (`packages/common/runtime/timing.ly`, D26). It
  writes, per event, the link, the moment and the length into a side file
  that both apps read. An engraver next to it can write the same for editing
  (§4).
- **Fast live compiles of unsaved buffers.**
  - A warm forked LilyPond (D25, D36) takes 0.3–0.9 s.
  - `LiveQueue` keeps one running compile and one replaceable waiting
    compile, so the newest edit wins.
  - The snapshot maps the links back to the real files.
- **Finding an href's places on the page in reading order** (`readingOrder`,
  D118), and choosing among a variable's several places (D120).
- **Spelling and writing notes.**
  - `notation.ts` spells pitches in a key and writes `\relative` bars with
    ties (D113).
  - `@lily/common/definition` finds a variable's definition through includes
    (D119).
- **Monaco as the editor.** Its tracked decorations and
  `executeEdits`/`pushUndoStop` give positions that follow edits, and an undo
  step per command (03 §6).

What is missing:

- a link gives only where a note *starts*;
- nothing says which voice a note is in, its pitch after `\relative`, or its
  duration when it is not written;
- nothing understands the tokens around a note;
- links are not mapped through edits made since the last compile. D19 calls
  this "Not compensated".

## 4. The spike: what a compile can say about each note

An engraver consisted into every `Voice` by an included settings file (as
`timing.ly` is) writes one row per notehead. On this score:

```lilypond
motif = \relative { c''8 d e4 }
\new Staff \relative {
  g'4 a8 b <c e g>2 |
  \motif \transpose c f \motif r4 |
  \new Voice = "low" { c,1 }
}
```

it wrote, among others:

```json
{"at":"score.ly:5:6",  "pitch":[0,5,0], "dur":"8", "moment":"1/4", "pos":-1}
{"at":"score.ly:5:9",  "pitch":[0,6,0], "dur":"8", "moment":"3/8", "pos":0}
{"at":"score.ly:2:20", "pitch":[1,0,0], "dur":"8", "moment":"1",   "pos":1}
{"at":"score.ly:2:20", "pitch":[1,3,0], "dur":"8", "moment":"3/2", "pos":4}
```

- **Pitch.** The written `b` is reported as an absolute pitch, octave 0, so
  `\relative` is already resolved.
- **Duration.** The `8` of `b` was not written; it came from the note before.
- **Transposed uses.** The `c` of `\motif` appears twice with one source
  place, as `c` and as `f`. The difference between written and sounding
  pitch is the transposition, and an edit made on the transposed use can be
  translated back.
- **Staff position.** `pos` is the staff position, which a ghost notehead
  needs (§6.5).
- **Voice.** The voice's id is empty for voices without a name. Rows need a
  key per context object instead.

Chapter 01 §5 adds what was tested further on 2.26.0:

- **Spans.** `ly:input-both-locations` gives a note event's *end* as well as
  its start, through its post-events (`c4-.`). The end is in tab-expanded
  columns; `span.ts`'s `columnToCharacter` converts them. Each post-event has
  its own span. In a chord each note's span is only its pitch.
- **Causes.** NoteHead, Rest, Script, Slur, DynamicText, KeySignature and
  TimeSignature have their event as their cause. Stem, Flag and Beam are
  caused by another grob. Clef, BarLine and StaffSymbol have no cause.
- **`output-attributes`.** It writes `id` and `data-*` attributes into the
  SVG, which `preview.js`'s sanitiser keeps. As a callback it is run once per
  broken piece, so a slur over a line break gets two ids. With every field in
  `data-*` attributes the SVG grew by 44 %. Compile time did not change
  measurably.
- **Which use of a variable.** A walk appended to
  `toplevel-music-functions`, which also works through `-dinclude-settings`,
  can set a property on every event: the chain of expressions around it,
  each with its span. Copies keep the property, the copies `\repeat unfold`
  makes included. An event inside `\motif` then knows the span of the
  `\motif` that used it, and the `\relative` and `\transpose` around that
  use.
- **The written pitch is not the event's pitch.**
  - Under `\transpose c d` the event says `fis` where the text says `e`.
  - An absolute variable used inside `\relative` is made relative again.
  - Edits therefore change the written token by a delta. They never write
    the event's pitch.
- **Clefs.** `\clef` makes no event and its origin is empty. A probe placed
  after each clef by the same walk reports its place and context.
- **Units.** One SVG unit is one staff space at the global staff size, and
  SVG's y is LilyPond's −y. This is what dragging needs.

## 5. Architecture

Text-first, in the shape chapter 03 calls *anchor → analyse → patch →
reconcile*.

### 5.1 The edit map (LilyPond side)

This is chapter 01's recommendation, in one runtime file passed with
`-dinclude-settings`. Whether two such files can be passed is not tested,
so it may have to be merged into `timing.ly`.

- **A walk of the music tree.** It is appended to `toplevel-music-functions`
  and sets a property on every event: its chain of enclosing expressions,
  each with its name and span. That chain names the use of a variable, the
  `\relative` or `\transpose` around it, and the repeat. The walk also places
  a probe after every `\clef`.
- **A Score-level engraver.** It acknowledges every grob whose cause, after
  following grob causes such as Beam → Stem → NoteHead, is an event. For each
  it records:
  - the grob's name;
  - the event's class, pitch and duration;
  - the event's span (`ly:input-both-locations`), its start in characters,
    and its chain;
  - the voice and staff, by id or as a numbered `Voice#n`;
  - the moment, the bar and the position in the bar.

  An `output-attributes` closure stamps `data-lily-id="N"` on the grob, or
  `"N.k"` on piece *k* of a broken one. At `finalize` the rows are written to
  `<output-name>.score-map.json`, keyed by id, next to `timing.json`.

This keeps the SVG small: an id per grob rather than every field in
attributes. A selection is found by its element, not only by its href, so
the two uses of a variable and the two passes of an unfolded repeat are told
apart. As with `timing.ly`, everything is wrapped in `catch`: a failure
loses the map, never the compile. The host converts the map's columns to
characters and maps it through the snapshot (D25). It treats the map as
untrusted, since a score can set `output-attributes` itself. The `textedit:`
links stay as they are, so D19 and D26 are untouched.

The Rust compiler (`crates/engrave`) and the extension's `compiler.ts` both
pass the file along, as they do `timing.json`. Both have to change, per
AGENTS.md's rule on the Rust port.

### 5.2 Reading the text (`@lily/common/music/`)

The edit map says where an event is and what it means. The text still has to
be read to know which characters are the pitch, the octave marks and the
duration, and what the next note inherits. This needs the local syntax of
music, not a parser of all of LilyPond:

- **Notes and rests.** Pitch, octave marks, octave check, `!`/`?`, duration,
  dots, `*n/m`, tie, and the post-events.
- **Chords and spacers.** `<…>` chords, `q`, `r`, `R`, `s`.
- **The note language.** `\language`, following includes as `definition.ts`
  does.

`\relative`, `\transpose` and variables need no simulation: the map gives
the previous note's real pitch and the chain around each use.

P0 chooses one of two ways to build it:

- **(a)** Swara's tree-sitter-lilypond through `web-tree-sitter`, which runs
  in both apps' renderers.
- **(b)** A scanner of our own in TypeScript, reading a window of text
  around a span that LilyPond has already found.

ly-lsp's `note_analyser.rs` is the reference for both. Either way it is
cross-checked against the map's spans on the corpus. In the longer run,
D56's hand-written parser is the one lossless parser the apps should share.

It is pure TypeScript, so both apps and the renderer use it (AGENTS.md's
rule for `packages/common`). Given a row of the edit map and the text, it
returns the target as one of three kinds:

| Kind | What it is | What an edit does |
| --- | --- | --- |
| **literal** | A note written once, where it is used | Edits it |
| **shared** | Inside a variable drawn or played more than once (`\repeat unfold`, a variable used twice, a transposed use) | The UI shows every other place first (D118's map already finds them) and says "changes all 4 uses". It also offers to change only this use: the variable is copied or unfolded first (ly-lsp's code action), or a tweak is written at the use. |
| **frozen** | Generated by a music function or Scheme, in a file the studio may not write, or a token the scanner does not recognise | Refused; the click goes to the code |

The rule is Overleaf's: what we don't understand, we don't edit.

### 5.3 Operations

Each operation is a pure function from the text, the target and the edit map
to a list of text edits. The functions are tested on text alone, the way
`notation.ts` is. LilyPond's context rules are applied as fix-ups:

- **Pitch is a delta on the written token.**
  - Up a step, up an octave, or to a named note: the written pitch is read
    from the token and changed by that much.
  - Under `\relative` the octave marks are spelled against the previous
    note's real pitch, taken from the map.
  - A changed pitch also changes what the *next* note means. Its octave
    marks are rewritten so that its real pitch stays the same (python-ly's
    `makeRelative`, 01 §1). In a chord, the chord's first note is the
    reference for what follows it.
- **Implicit durations.** Changing or deleting a note whose duration the
  next note inherits writes that duration on the next note (python-ly's
  `rhythm_implicit`, 01 §1).
- **Transposed uses.** Because the delta is applied to the written token,
  an edit made on a transposed use changes the variable by the same
  interval.
- **Accidentals.** Spelled in the key of the moment (`notation.ts`'s `spell`)
  and in the score's `\language`.

### 5.4 Applying an edit (renderer)

- **Undo.** One user command is one `pushUndoStop(); executeEdits(…);
  pushUndoStop()`, on the editor's own undo stack. It never uses `setValue`.
  Undo and redo then serve both views.
- **Stale links.** On every render, each row's span becomes a Monaco tracked
  decoration, so its link follows the text through later edits until the
  next compile. Frescobaldi's `BoundLinks` does the same with `QTextCursor`
  (01 §2). This also fixes D19's "positions are those of the last compile"
  for click-to-source. An edit whose target was deleted since the render, or
  whose token no longer reads as expected, is refused until the next render.
- **No sync loop.** While an edit is applied, the cursor does not move the
  score's highlight (mei-friend's `blockChanges`, 02 §1.2).
- **Selection.** Kept as source spans and voice keys, not SVG elements. It
  is selected again on each new render.

### 5.5 Feedback within the compile time

LilyPond cannot lay out a change in less than a frame, so the edit is shown
before it is engraved:

- The edited elements are drawn over the last real page: a ghost notehead
  at the new staff position, a faded deletion, a marked insertion.
- The compile runs through the existing `LiveQueue`.
- The real page replaces the overlay. If the compile fails, the last good
  page stays, marked as out of date.
- Holding ↑ moves only the ghost on each key down, and compiles on key up.
  This is verovio-editor's preview-then-commit (02 §3.2) and
  Sketch-n-Sketch's "heavy work at mouse-up" (03).
- No local re-layout is attempted. That is D56's engine's work. If it is
  built, the same operations get frame-time feedback without changing.

### 5.6 Input

Copied from what makes MuseScore and Dorico fast (02 §7):

- **Selection.** Click, shift-click, a range across voices, and a rectangle.
  The text editor's selection follows: one cursor per selected note.
- **Moving.** ←/→ moves through the voice by moment; ⌥↑/⌥↓ moves to the
  staff above or below.
- **Changing.** ↑/↓ changes pitch, ⌘↑/⌘↓ the octave, digits the duration
  (as in MuseScore), `.` adds a dot, `0` turns a note into a rest, ⌫ deletes.
- **Note input.** A caret after the selection. Letters `a`–`g` insert notes
  in the octave nearest the previous one, which is what `\relative` already
  means. Notes are inserted, never overwritten: LilyPond has no pre-filled
  bars.
- **Popovers** in Dorico's way, but in LilyPond's own syntax.
  - A key opens a small field: dynamics (`\mf`), articulation (`-.`), clef,
    key or time.
  - What is typed there is inserted as typed. It is checked by the scanner
    and by the next compile.
- **MIDI keyboard input,** later. It needs key-aware spelling and the Rust
  side (`midir`), since WKWebView likely lacks Web MIDI. Not verified
  (02 §8).

### 5.7 Where the code goes

| Part | Location |
| --- | --- |
| The scanner, the operations and the target classification | `packages/common/src/music/`, with no Node or `vscode` |
| The selection, the overlay and hit testing over the SVG | `packages/common/web/` beside `preview.js`, which both apps load |
| The key bindings and the Monaco side | Studio's renderer |
| The engraver | `packages/common/runtime/` |

Studio comes first, because it owns its editor. The extension follows by
posting the same edits from its webview and applying them as a
`WorkspaceEdit`.

## 6. Phases

The effort is a rough estimate for one engineer working with agents. It is
not measured.

| Phase | Scope | Gate to go on | Effort |
| --- | --- | --- | --- |
| **P0 Spike** | The score map (§5.1) with ids, chains and clefs; choosing (a) or (b) of §5.2; a click and ↑ change a pitch in the text, in Studio, behind a flag | On the repository's sample scores and the D118/D120 scores, ≥ 95 % of noteheads resolve to a literal or shared target that the reader reads. The time from a key press to the new page is measured. The map works in the warm, forked worker of D25. | 1–2 weeks |
| **P1 Selection** | Selecting on the score and in the text, both ways; moving through voices; tracked decorations for stale links; target kinds shown on hover. As a first deliverable, Frescobaldi's way: a selection on the score becomes a text selection, and existing text commands (transpose, articulations, the agent's context menu, D41) act on it. | Selection survives typing and recompiles; no sync loops | 1–2 weeks |
| **P2 Editing notes** | Pitch, accidental, octave, enharmonic spelling, duration, dots, rest↔note, delete, tie; `\relative` and implicit-duration fix-ups; shared targets; one undo step each | A corpus test: every operation on every literal note of the scores leaves every *other* note's absolute pitch and duration unchanged, judged from the next compile's edit map | 3–5 weeks |
| **P3 Feedback** | The ghost overlay, preview on key down and compile on key up, out-of-date state | Every edit shows visible feedback within one frame | 1–2 weeks |
| **P4 Marks** | Articulations, dynamics, slurs and hairpins from popovers and a palette; editing and deleting the post-events of a note | — | 2–3 weeks |
| **P5 Note input** | The caret, letters and digits, chords, inserting rests and bars | Entering a bar of eighths in a new voice needs no text typing | 3–4 weeks |
| **P6 Dragging layout** | `\tweak extra-offset`, `\offset` and `\shape` written at the use. An existing tweak is replaced, never stacked (Liska's warning, 01 §6). Spontini's arithmetic is used, in staff spaces of the grob's own staff, but not its ids. Slur control points are read from `output-attributes`. | — | 3+ weeks; optional, and where every predecessor stalled |
| **P7 The extension** | The same, in VS Code's webview | — | 2 weeks |

P0–P3 make a useful first release: notes are selected and changed on the
score. That is 6–11 weeks.

P2's corpus test is the protection that matters. An edit is checked by what
the next compile says, not only by what text it wrote.

## 7. Risks and open questions

- **What the scanner does not cover.** User music functions that take music
  and change it (`\appoggiatura`-like functions of the score's own) still
  produce links to the written notes. An edit there writes the right text,
  but its effect on the page may be surprising. P0 measures how often this
  happens.
- **Latency.** 0.3–0.9 s per compile is acceptable for selecting and
  changing notes, with the overlay. It is sluggish for note entry at speed.
  A large score may need to compile only the visible region (mei-friend's
  "speed mode"; LilyPond's `showFirstLength`/`skipTypesetting`). That is not
  measured.
- **Lily Studio on the web** (D55) compiles on the server, so the round trip
  adds network time there.
- **Two compilers.** The edit map has to be passed by both the Rust and the
  TypeScript compiler.
- **`\language` and includes.** Note names depend on a language set in
  another file. The scanner follows includes as `definition.ts` does; the
  include path `-I` is not followed, as there.
- **Undo across files.** An edit inside an included `.ily` lands in that
  file's model and its own undo stack.
- **Not surveyed in LilyPond** (01, "Not yet verified"):
  - grace-note moments;
  - lyrics, chord names and figures;
  - `\partcombine` and cross-staff notes;
  - `\fixed`;
  - `\book` and `\bookpart`;
  - music made by Scheme without an origin.

  P0's corpus has to include them, or the scope has to exclude them.
- **The map's cost.** On a 4 × 120-bar score, ids and a side file are
  expected to cost far less than the 44 % that `data-*` attributes cost.
  This is not measured.

## 8. Decision to record

When P0 passes its gate, a D125 in `docs/DECISIONS.md` should record:

- edits on the score are patches of the text;
- the edit map comes from an engraver;
- targets are literal, shared or frozen;
- one command is one undo step.
