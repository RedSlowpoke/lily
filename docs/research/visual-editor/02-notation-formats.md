# 02: Visual editing over text notation formats, and GUI note-input models

Research for Lily's planned visual editor: edits made on the rendered LilyPond SVG and written back into the `.ly` source. Date: 2026-10-26.

**How to read the evidence tags**
- **[V-src]**: verified by reading the project's source code (shallow clones made 2026-10-26; commit given per project).
- **[V-doc]**: verified from official documentation, release notes, or the project's own issue tracker / PRs.
- **[K]**: my general knowledge, not re-checked this session.
- **[I]**: my inference or recommendation.

---

## 0. Summary table

| Tool | Text format | Score→text mapping | How an edit reaches the text | Refresh after an edit | Undo | Licence |
|---|---|---|---|---|---|---|
| **mei-friend** | MEI XML (CodeMirror 5) | `xml:id` on every element = SVG `g#id` (Verovio keeps ids) | Parse the whole buffer to a DOM, mutate the node, serialise **that element only**, regex-find it in the buffer by `xml:id`, replace that span | Full Verovio re-render after a 400 ms debounce. "Speed mode" renders only the current page plus dummy context pages | The text editor's own undo (CodeMirror history), so text and visual edits share one stack | AGPL-3.0 |
| **Verovio Humdrum Viewer (VHV)** | Humdrum `**kern` (Ace) | Positional SVG ids `note-L12F3S2` (line/field/subtoken) | Replace one tab-separated token on one line. Tabs are preserved | Full Verovio re-render of the whole buffer on every change, no debounce | Ace undo (Ctrl-Z is routed to the editor even when the score has focus) | No licence file (all rights reserved by default) |
| **Neon** | MEI (neumes); no text pane | Verovio element ids | Verovio **editor toolkit** mutates the in-memory model; MEI is regenerated with `getMEI` | Full SVG re-render of the page; optimistic DOM translate while dragging | Snapshot stack of the whole MEI string, 10 deep per page | MIT |
| **Verovio editor toolkit / verovio-editor** | MEI; the XML pane is `setValue`'d with regenerated MEI | Element ids | Model-first (C++); MEI is re-serialised | Two tiers: `redoPagePitchPosLayout` (cheap preview, uncommitted) and `commit` + `redoLayout` | Internal snapshot stack (MEI strings), memory-trimmed | Verovio LGPL-3.0; the editor app AGPL-3.0 |
| **abcjs** | ABC | `abcelem.startChar` / `endChar` (character offsets) | The app splices the string itself; abcjs never edits the text | Full re-render (the Editor uses a 300 ms debounce) | Whatever the textarea / app provides | MIT |
| **EasyABC** | ABC | abcm2ps `-A` annotations give row/col per note; hit-testing by geometry, plus rubber-band selection | Text-level actions on the selection (assist panel, toolbar inserts, abc2abc for transposition) | Full abcm2ps re-render; manual F5 mode for long tunes | Editor undo | GPL-2.0 |
| **alphaTab / alphaTex** | alphaTex | AST nodes carry start/end source locations (1.7); no documented click-to-source | No visual editing. The exporter regenerates the whole document; the AST drops whitespace | n/a | n/a | MPL-2.0 |
| **VexFlow / VexTab** | VexTab / none | none | Render-only | n/a | n/a | VexFlow MIT; VexTab non-commercial |
| **Guido** | GMN | Graphic ↔ **time** map (`GuidoGetMap`), not graphic ↔ text | Render-only | n/a | n/a | MPL-2.0 |
| **OSMD** | MusicXML | data model only | "not an interactive sheet music editor" | n/a | n/a | BSD-3 |
| **Strudel** | JS / mini-notation (CodeMirror 6) | Each event ("hap") carries `context.locations` (offsets) | Inline widgets write the text **and** poke runtime state directly (fast path) | Highlights move with the text via `locations.map(tr.changes)` until the next eval | CodeMirror history | AGPL-3.0 |

---

## 1. mei-friend: the closest analogue (deep dive)

Repo `mei-friend/mei-friend` @ `b5976af` (2026-07-22). Vanilla JS, CodeMirror 5, Verovio in a Web Worker. The paper is Goebl & Weigl, JOSS 9(98) 6002, 2024: <https://joss.theoj.org/papers/10.21105/joss.06002>. It began as an Atom package and was rewritten as a web app: <https://teimec2023.uni-paderborn.de/contributions/159.html>.

### 1.1 Selection mapping, both directions [V-src]
- **Score → text.** `Viewer.addNotationEventListeners` attaches a click handler to every `g[id], rect[id], text[id]` inside `.page-margin` (`app/static/lib/viewer.js` ~L551). `handleClickOnNotation` takes `event.currentTarget.id`. Alt-click promotes the id to its enclosing `<chord>`; clicking a tuplet number selects the `<tuplet>`; Cmd/Ctrl-click toggles the id in a multi-selection. Then `utils.setCursorToId(cm, id)` runs a **regex search for `xml:id="ID"` in the text buffer**, moves the cursor to the tag start, and scrolls the editor so the line is vertically centred (`utils.js` L217).
  - When the id is not found in the text (Verovio generated it because the MEI has none), mei-friend shows "missing ids" and offers *Manipulate → Add ids* (Cmd-M).
  - **Every element must carry an `xml:id` for the round trip to work.**
- **Text → score.** On CodeMirror `cursorActivity`, `getElementIdAtCursor` scans the lines around the cursor to find the enclosing tag and its `xml:id`. If that tag has none, it walks up to the parent `<staff>` or `<measure>` id. The SVG element `g#id` is then highlighted and scrolled into view. If the element is on another page and "flip page" is on, Verovio is asked for that page (`viewer.js` L667–697).
- **Keyboard navigation in the score works on the SVG geometry, not the text.** `navigate()` gets the selected `g#id`, reads its x/y, and finds the next or previous element in the measure with `dutils.getIdOfNextSvgElement`. Up/down between layers sorts candidates by |Δx| and then y. Past the page edge it flips the page (`viewer.js` L2432ff).

### 1.2 How edits are applied to the text [V-src]
Every command in `editor.js` follows the same pattern. `shiftPitch` (L934) is a typical one:

```js
v.loadXml(cm.getValue());                 // re-parse the WHOLE buffer into an XML DOM
v.allowCursorActivity = false; cm.blockChanges = true;   // mute the sync loops
for (id of selected) { el = xmlDoc.querySelector('[*|id="'+id+'"]');
  replaceInEditor(cm, pitchMover(v, el, delta), true); }  // mutate DOM node, then patch text
addApplicationInfo(v, cm);                // stamps <application> in meiHead
cm.blockChanges = false; v.allowCursorActivity = true;
handleEditorChanges();                    // → debounced re-render
```

- `replaceInEditor(cm, xmlNode)` (L2148) serialises **only the mutated element** with `xmlToString`. It builds a regex `<note …xml:id="ID"…/>` (self-closing) or `<note …xml:id="ID"…>…</note>` (lazy match to the closing tag), runs it through `cm.getSearchCursor`, and replaces the match. It then re-indents the replaced lines with `cm.indentLine(l,'smart')`, deletes lines left empty, and selects the new text.
- `addNote` (L211) inserts after the selected element by going to its matching close tag and calling `replaceRange('\n'+xml)`. The new note copies `pname/oct/dur/dots/...` from the selected one and gets a freshly generated `xml:id`.
- `removeInEditor` deletes by the same regex.
- **What this means for user formatting.** Formatting outside the edited element survives byte for byte. Inside it, attribute order, quoting and whitespace are re-serialised, and comments inside a replaced element survive only if the DOM kept them. The lazy `[\s\S]*?</note>` regex is fragile with nested elements of the same name [I].
- **Cost.** Every edit re-parses the whole document with `DOMParser` (`v.loadXml(cm.getValue())`). mei-friend's docs say slowdowns start beyond ~25k lines of MEI [V-doc].

### 1.3 Refresh and latency [V-src, V-doc]
- `handleEditorChanges()` → `v.notationUpdated(cm)` → **debounced 400 ms** (`defaultNotationUpdateDelay = 400`, `defaults.js` L591). Then a cheap `DOMParser` well-formedness check: on malformed XML the view is marked "Rendering paused — waiting for valid XML" instead of erroring. Then `updateData` sends the MEI to Verovio in a worker. Score edits pay the same debounce as typed text.
- During the edit, the flags `cm.blockChanges` and `v.allowCursorActivity = false` stop the text→score sync from re-entering. This feedback-loop guard is essential, and Lily will need the same thing [V-src].
- **Speed mode** [V-doc: <https://mei-friend.github.io/docs/advanced/largefiles/>; V-src `speed.js` header]: "only transfers a single-page excerpt encompassing the most recently selected element to Verovio". The excerpt is built from the DOM, wrapped in invisible dummy pages before and after "to ensure context (time-spanning elements, key signatures, tempo…)". It relies on encoded `<sb>/<pb>` or on breaks precomputed on load. Spanners are precomputed in a separate worker. There is also a manual "update" button mode.

### 1.4 Undo [V-src, V-doc]
- Undo is CodeMirror's own history (`cmd.undo = () => cm.undo()`), so visual and text edits share one stack and Cmd-Z works from either pane.
- **Complaint, issue #179 (2026)** <https://github.com/mei-friend/mei-friend/issues/179>: undo "reverts not the whole action but … to a temporary state inside of the action… chromatically moving up a note takes four Undos". One visual command made several text changes (replace, re-indent, delete empty lines, `<application>` stamp), and each became its own undo step.
- **Issue #135** <https://github.com/mei-friend/mei-friend/issues/135>: "Surround with tag does not create an undo checkpoint", so undo removed too much.
- I found no `cm.operation()`-style grouping in `lib/` [V-src grep]. **Lesson: wrap each visual command in one atomic, single-undo-stop transaction.**

### 1.5 Input model [V-src: `app/static/keymaps/default-keymap.js`]
The input model is selection-based modification, not caret entry. With focus on the notation:

| Key | Action |
|---|---|
| ←/→ | next / previous note |
| ↑/↓ | layer up / down |
| Shift-↑/↓ | pitch chromatically |
| Shift-Alt-↑/↓ | pitch diatonically |
| Cmd-Shift-↑/↓ | octave |
| Shift-←/→ | duration |
| `.` | dots |
| `n` | add note after (copies the previous one) |
| Shift-N | to rest |
| `c` | chord |
| `+ = -` | accidentals |
| `s` / `t` | slur / tie |
| `h` | hairpin |
| `d` | dynamics |
| `f` | fermata |
| Shift-S/V/E/A | stacc / accent / tenuto / marcato |
| `b` | beam |
| `o` | 8va |
| `x` | flip placement |
| Alt-arrows | visual offset `@ho/@vo` (layout tweak, written as attributes) |
| Delete | delete |
| Shift-Space | switch focus between text and notation |

There is no MIDI note input.

### 1.6 Other known complaints [V-doc]
- **#114**: pitch moves ignore earlier accidentals in the measure, so `@accid.ges` ends up wrong. Context-dependent semantics are hard to get right in local text patches. <https://github.com/mei-friend/mei-friend/issues/114>
- **#192**: paste duplicates `xml:id`s and causes parser errors. Identity-by-id breaks on copy/paste. <https://github.com/mei-friend/mei-friend/issues/192>

### 1.7 Reuse
AGPL-3.0, so **no code reuse** in an MIT app. The ideas are free to copy.

---

## 2. Verovio Humdrum Viewer (VHV)

Repo `humdrum-tools/verovio-humdrum-viewer` @ `cd10c8a` (2026-09-11). Jekyll-assembled plain JS in `_includes/scripts/`, Ace editor, Verovio (with its Humdrum importer) in a worker.

- **Mapping [V-src].** Verovio's Humdrum importer emits positional ids such as `note-L12F3S2`: line 12, spine/field 3, subtoken 2 for chord notes. Ids for spanning elements carry two positions.
  - *Score → text:* `processNotationKey` parses `L(\d+)`, `F(\d+)`, `S(\d+)` out of the clicked element's id (`editor/processNotationKey.js`). `highlightIdInEditor` converts line/field into an Ace column by counting tabs.
  - *Text → score:* on Ace `changeCursor`, `humdrumDataNoteIntoView` computes field and subspine from the cursor column, builds the query `L{line}F{field}[S{n}]`, and selects `g[id$='…']` in the SVG, preferring elements with the class `qon` (`utility-ace.js` L186ff).
  - This is the **same shape as LilyPond's `textedit://` links**: positional, not stable ids. It works because ids are regenerated on every render.
- **Edits are token-level text patches [V-src].**
  - `getEditorContents(line, field)` extracts the tab-delimited token.
  - Commands like `transposeNote`, `toggleStaccato` or `toggleSharp` regex-rewrite only that token. For example, staccato appends `'` after the pitch letters: `token.replace(/([a-gA-G]+[-#nXxYy<>]*)/g, p1 => p1+"'")`.
  - `setEditorContents(line, field, token)` rebuilds the line **preserving the exact tab runs** and does one `EDITOR.session.replace(range, newline)`.
  - The global `FreezeRendering` flag batches a multi-token command into a single render. `RestoreCursorNote = id` re-selects the same element after the re-render; that works because the positional id is unchanged when no lines move.
  - Comments and unrelated formatting are untouched.
- **Scope of graphical editing [V-doc via the CCARH wiki, quoted in search results; <https://wiki.ccarh.org/wiki/Vhv>].** "semi-graphical: some elements such as pitch, beaming, slurs, articulations can be changed graphically, while other elements such as lyrics, adding music, changing rhythms, can only be done by editing the digital score in the text editor."
- **Refresh [V-src].** Every Ace `change` → `monitorNotationUpdating()` → `displayNotation()` re-renders the full document. There is no debounce; it is gated only by `FreezeRendering`. **Undo:** Ctrl/Cmd-Z in the score calls `EDITOR.undo()` (`listeners/processNotationKeyCommand.js` L29–33).
- **Keys [V-src].**

  | Key | Action |
  |---|---|
  | `#` `-` `n` | sharp / flat / natural |
  | `'` | staccato |
  | `^` | accent |
  | `~` | tenuto |
  | `s` | slur |
  | `a` / `b` | stem up / down |
  | `y` | toggle visibility |
  | `L` / `J` | beam start / end |
  | `m` / `M` / `w` | mordents |
  | `q` | grace |
  | `p` | pedal |
  | transpose up/down step and octave | (separate commands) |

  A numeric prefix (`InterfaceSingleNumber`) repeats a command over N following notes, vi-style.
- **Licence.** None in the repo (GitHub reports no licence), so treat it as all rights reserved. Ideas only.

---

## 3. Neon (DDMAL), and Verovio's editor toolkit

### 3.1 Neon
Repo `DDMAL/Neon` @ `165574e`, TypeScript, MIT. Square-notation (neume) correction over facsimiles; there is no text pane.
- Edits are JSON **editor actions** sent to Verovio's `edit()` in a worker: `drag {elementId,x,y}`, `insert`, `set`, `merge`, `split`, `chain`, and so on (`src/Types.ts`). Actions are documented at <https://github.com/DDMAL/Neon/wiki/Toolkit-Actions>.
- After an action, `getMEI` regenerates the whole document [V-src].
- **Undo [V-src, `NeonCore.ts`].** A per-page stack of **entire MEI strings**, capped at 10 (`if (len > 10) slice(1)`). Undo means `loadData(previousMEI)`.
- **Drag [V-src, `utils/DragHandler.ts`].** While dragging, Neon translates the selected SVG nodes optimistically. On drop it sends one `chain` of `drag` actions, then re-renders the whole SVG. A code comment notes that the re-render "drops the `selected` class from every node, so record what to reselect NOW". It re-selects by id afterwards and tolerates ids that disappeared.
- Formatting and comments are lost (model regeneration). That is irrelevant for Neon, and fatal for Lily.

### 3.2 Verovio editor toolkit (C++)
`rism-digital/verovio`, LGPL-3.0. A major API update, PR #4415 (merged 2026-08-18) <https://github.com/rism-digital/verovio/pull/4415>, and PR #4487 (2026-10-14) added a full **CMN cursor-entry model** [V-src `src/editortoolkit_cmn.cpp`, `editortoolkit_shared.cpp`; V-doc PR text]:
- `setCursor {elementId, inputMode:"durationFirst"|"pitchFirst", chordMode, restMode}`
- `insertCursorByPitch {pname, oct, accid}` or `{midi: 60}`. Accidental spelling is computed from the key signature and preceding accidentals (`GetActualAccid`).
- `insertCursorByDur {dur, dots}`
- `insertCursorByType {type:"rest"|"copy"|"tie"}`
- `insertCursorContainer` (beam, graceGrp, tuplet), plus an auto-beam option
- `keyDown` (↑/↓ pitch step, Ctrl ×7 for an octave, ←/→ duration)
- `navigate`, `insertMeasure`, `setScoreDef*`, `undo`, `redo`, `commit`
- **Undo** = a snapshot of the MEI serialisation (`GetCurrentState()` → `MEIOutput`) per committed action, with memory trimming.
- **Two-tier refresh** (PR text): "`redoPagePitchPosLayout()` – Recalculate pitch positions on the current page, used for non-committed preview edits"; "Use `commit` for document-changing edits that should enter the undo history"; "`redoLayout()` when an action can change pagination".
- **verovio-editor** (`rism-digital/verovio-editor` @ `3a0213f`, AGPL) uses this pattern [V-src `ts/events/action-manager.ts`]. On **keyDown**, the edit is applied and only `editRefresh()` runs (cheap pitch-position relayout plus SVG); on **keyUp** it runs `commit` (full layout and one undo entry). Holding ↑ therefore previews many steps and commits once. Its XML pane is just `setValue(regeneratedMEI)`.
- Also relevant: open PR #4375 adds `getPitchPosition(scoreTime, midiPitch, staff)`, which returns SVG (x, y) for a pitch at a time and is meant for overlays and insertion cursors <https://github.com/rism-digital/verovio/pull/4375>.

---

## 4. abcjs and EasyABC

### abcjs
`paulrosen/abcjs` @ `2e399a6`, MIT.
- **Mapping [V-src].** Every rendered element's `abcelem` has `startChar` / `endChar` into the source string. `abcjs.Editor.highlight` calls `editarea.setSelection(abcelem.startChar, abcelem.endChar)` (`src/edit/abc_editor.js` L399). The reverse, `rangeHighlight(start,end)`, highlights every element whose char range overlaps the text selection (`src/write/interactive/selection.js` L367).
- **Editing back [V-doc <https://docs.abcjs.net/visual/dragging.html>].** With the `{dragging:true}` option a note can be dragged vertically. The click listener then receives `drag.step` ("a purely visual count" of staff positions), `drag.index`, `drag.max` and `drag.setSelection`. "The underlying music is not changed… The clickListener function should modify the ABC string and rerender."
  - The official example (`examples/dragging.html`) tokenises `abcString.substring(startChar,endChar)`, shifts only the pitch tokens, splices the string, and redraws in full.
  - Notes are keyboard-focusable: the SVG gets `tabindex=0` with a keydown handler (`selection.js` L10–12).
- **Refresh [V-src].** `Editor.fireChanged` re-parses and debounces the redraw by 300 ms ("Is this a good compromise…?").
- **Reusable.** Yes (MIT), but it is ABC-specific.

### EasyABC
`jwdj/EasyABC`, GPL-2.0, wxPython desktop app.
- **Mapping [V-src].** It runs **abcm2ps with `-A`** (annotate), which emits source row/col for each note in the SVG (`easy_abc.py` L967–969). `svgrenderer.py` parses these into `NoteData(note_type,row,col,x,y,w,h)`. Hit-testing is by nearest distance; rectangle selection finds every note inside a rubber band (L274–311).
  - **This is the closest architectural twin to LilyPond point-and-click:** an external engraver, annotated SVG, and positional source refs.
- **Editing [V-src/V-doc <https://easyabc.sourceforge.net/>].**
  - Select a note, then pick a symbol from the toolbar; the symbol is inserted as text.
  - An "ABC assist" panel (`tune_actions.py`: `PitchAction`, `DurationAction`, `AccidentalChangeAction`, `KeySignatureChangeAction`, …) offers context-sensitive actions for the element under the **text** cursor and rewrites the text.
  - Transpose / note length go through `abc2abc` on the selection.
  - Documented limits:
    - "These functions may not work well with chords yet."
    - "For very long tunes, you can turn off automatic score updates and refresh manually with F5."
    - Arrow-key cursor moves do not update the score highlight "for performance reasons".

---

## 5. alphaTab / alphaTex, VexFlow / VexTab, Guido, OSMD

- **alphaTab** (MPL-2.0) [V-doc <https://alphatab.net/docs/releases/release1_7>]
  - Version 1.7 replaced the importer with an AST-based parser. The AST "does not retain any whitespace information" but keeps comments and punctuation.
  - It ships an LSP (`@coderline/alphatab-language-server`) with completion, hover and diagnostics, a Monaco integration that runs it in a Web Worker <https://alphatab.net/docs/alphatex/monaco>, and a prototype VS Code extension with live preview.
  - The exporter "export[s] the whole data model into an alphaTex document": regeneration, with no formatting preservation documented.
  - **No visual editing and no documented click → source.** alphaTab exposes note/beat mouse events and a bounds lookup on its model [K].
  - Lesson: the same "LSP + live preview" shape Lily already has, and nobody has done write-back.
- **VexFlow** (MIT) is a rendering library with no source text. **VexTab** (non-commercial licence) is a text → render tutorial editor with no write-back [K, licence V-doc].
- **Guido** (MPL-2.0) [V-doc <https://guidolib.sourceforge.io/doc/guidolib/classGuidoScoreMapAdapter.html>]. `GuidoGetMap` / `GuidoScoreMapAdapter` map **graphic rectangles ↔ time segments** (`getTime`, `getPoint`), not source text. The Guido editors are text + preview with no score → text edits [I].
- **OSMD** (BSD-3) [V-doc README]: "OSMD is a renderer, not an interactive sheet music editor. Rendering takes some time, and you can't easily/quickly move notes, place new notes". It suggests mutating SVG nodes directly for instant changes such as colour.

## 6. Strudel / TidalCycles: code with live, inline widgets

`uzu/strudel` @ `064cf07`, AGPL-3.0, CodeMirror 6.
- **Mapping [V-src `packages/codemirror/highlight.mjs`].** At eval time the transpiler attaches source `locations` (start/end offsets) to every event (`hap.context.locations`). Every animation frame the active haps are highlighted with `Decoration.mark`.
- Between evals, the stored locations are **remapped through each text change**: `locations = locations.map(tr.changes)`. This keeps highlights correct while the user types, without re-evaluating.
- **Inline widget write-back [V-src `slider.mjs`].** `slider(0.5, 0, 1)` in the code is replaced by an `<input type=range>` widget. On input it does two things:
  1. `view.dispatch({changes:{from, to, insert}})`, so the **text stays the source of truth**;
  2. `postMessage({type:'cm-slider', id, value})`, which updates the running pattern's value **without re-evaluating**: a fast path with the text written in parallel.
- Widget positions are also remapped through `tr.changes`.

---

## 7. GUI note-input models worth copying

### MuseScore Studio 4.x [V-doc <https://handbook.musescore.org/basics/entering-notes-and-rests.md>, <https://handbook.musescore.org/basics/input-by-duration-mode.md>, <https://handbook.musescore.org/basics/alternative-note-input-methods.md>]

**Modes.** Default "Input by note name": pick a duration, then type the pitch. "Input by duration" (`M`): pitch first, then duration.

**Durations.** Digits `1`–`9`, with `4`/`5`/`6` = eighth/quarter/half ("the middle row of a numeric keypad").

**Pitch.**
- Letters `A`–`G`. "MuseScore will choose the octave that is closest to the previous note on that staff". **This is exactly LilyPond's `\relative` rule.**
- `Ctrl+↑/↓` changes the octave; `↑/↓` moves a semitone; `J` respells.
- Chords: `Shift+letter` adds a chord note; `Alt+1..9` adds an interval above.

**Rests and modifiers.**
- `0` enters a rest; `.` adds a dot; `T` ties.
- `- + =` set accidentals, which by default apply to the *next* note entered.

**Overwrite vs. insert.**
- Note entry *overwrites* rests in pre-filled measures; that model depends on fixed-length bars.
- An **Insert** method, or `Ctrl+Shift` + note, pushes later music forward.
- **Re-pitch** (`Ctrl+Shift+I`) keeps the rhythm and replaces pitches.
- Other methods: **Rhythm**-only, and **Real-time** (metronome or foot pedal).

**Other input.** MIDI keyboard and an on-screen piano panel (`P`).

### Dorico [V-doc, partially; Steinberg help pages are JS-rendered and only partly fetchable]

**Caret entry.**
- `Shift+N` (or Return) starts note input with a **caret** on the staff and rhythmic-grid ticks above it.
- Durations are `6` quarter, `5` eighth, `4` sixteenth, `7` half; letters enter pitches in the prevailing key.
- `Space` advances the caret by the current duration without entering a note, and the gaps become implicit rests. <https://steinberg.help/dorico_se/v5/en/dorico/topics/write_mode/write_mode_note_input/write_mode_note_input_inputting_notes_t.html>

**Popovers.** A small text field opens at the caret or selection; you type a mini-language and press Return:
- `Shift+D` dynamics (`p`, `mf`, `p<f>p`, `cresc.`)
- `Shift+K` key (`3s`, `4f`)
- `Shift+M` metre (`3/4`, `cut c`)
- `Shift+T` tempo
- `Shift+C` clefs (`tenor`)
- `Shift+O` ornaments
- …and others for tuplets, bars, repeats, lyrics and playing techniques.

Return on an existing item reopens its popover to edit it. Alt+Return applies to the caret's voice only. Sources: <https://www.steinberg.help/r/dorico-pro/5.1/en/dorico/topics/write_mode/write_mode_notations_input/write_mode_dynamics_inputting_popover_t.html>, <https://www.steinberg.help/r/dorico-pro/5.1/en/dorico/topics/write_mode/write_mode_notations_toolbox_r.html>.

**Why it is fast [K].**
- Bars are not containers; music is a stream and barlines are derived from the metre, which is close to LilyPond's model.
- An explicit Insert mode (`I`) and chord mode (`Q`) are toggles.
- Popovers need no mouse and no palettes, and the syntax is guessable.

### Sibelius [K]
- Select, then type a letter or a keypad duration. The on-screen Keypad mirrors the numeric keypad in layouts (F7–F12).
- `Alt/Opt+1..9` adds an interval above.
- `Ctrl/Cmd+E` opens expression text, with a word menu where typing `mf` gives the glyph; `Ctrl+T` technique; `S` slur; `H`/`Shift+H` hairpins; `L` lines gallery.
- Command search in the ribbon.
- **Fast because:** almost everything is one keystroke on a selection, and text items accept typed shorthand.

### Finale [V-doc for the end of life; K for entry]
- Development ended 26 Aug 2024 <https://www.synthtopia.com/content/2024/08/26/finale-ends-after-35-years/>.
- *Speedy Entry*: a caret in the measure, ↑/↓ moves pitch, number keys enter durations, and the caret is MIDI-aware.
- *Simple Entry*: letters plus modifiers.
- A lesson in what not to do: many modal tools, each with its own keys.

---

## 8. Cross-cutting findings

1. **Two families exist, and only one fits Lily.**
   - *Text-patch* tools (mei-friend, VHV, abcjs, EasyABC, Strudel) keep the text as truth and preserve everything outside the patched span.
   - *Model-first* tools (Neon, the Verovio editor toolkit, alphaTab's exporter, model-to-LilyPond GUIs [K]) regenerate the text and lose formatting, comments and abstractions.
   - Nobody successfully round-trips a *regenerated* text format while keeping user formatting. Every text-centric editor patches spans [V-src across projects].
2. **Patch granularity decides how much formatting survives.** VHV (one token, tabs preserved) and the abcjs example (pitch sub-tokens only) are the gold standard. mei-friend's whole-element re-serialisation is a known compromise [V-src].
3. **Positional ids (VHV, EasyABC, LilyPond `textedit://`) work if you re-render after every edit** and re-select by the *new* position. VHV gets away with "same id" because pitch edits rarely move lines. Strudel shows the general fix: map the stored positions through the text change set [V-src].
4. **One visual command must be exactly one undo step in the shared text undo stack.** mei-friend #179 and #135 are the cautionary tale [V-doc].
5. **Latency hiding.**
   - Neon translates nodes optimistically while dragging.
   - The Verovio toolkit splits a cheap preview relayout from an expensive commit, and verovio-editor commits on keyup.
   - Strudel updates runtime state without re-eval.
   - mei-friend renders only the visible page with context.
   - Debounces: mei-friend 400 ms, abcjs 300 ms, VHV none [V-src].
6. **Context-dependent semantics bite local patches.** Examples: accidentals in the measure (mei-friend #114), key spelling (Verovio `GetActualAccid`), and in LilyPond `\relative` octaves [I].
7. **Graphical editing scope is deliberately partial in the successful tools.** VHV does pitch, beaming, slurs and articulations graphically, and rhythm changes, insertion and lyrics in text [V-doc].

---

## Lessons for Lily (ranked, concrete)

1. **Patch the text at the token level; never regenerate.** Each visual command becomes a minimal `TextEdit[]` computed from Lily's own LilyPond tokenizer around the clicked `textedit://FILE:LINE:CHAR:COL` start position.
   - Rewrite only the pitch letters, octave marks, duration digits or a single post-event (`-.`, `\p`, `(`), as VHV and the abcjs example do.
   - LilyPond links give a *start* point only. Unlike abcjs's `startChar/endChar`, the token's extent has to come from Lily's parser [I; link format per the brief].
   - Comments, spacing, variables and `\relative` blocks survive by construction. This is the only approach every successful text-centric tool shares [V-src].
2. **One command = one atomic Monaco edit = one undo stop.**
   - Use `pushUndoStop()`, then a single `executeEdits`/`pushEditOperations` with all changes, then `pushUndoStop()`.
   - Leave metadata stamps out of the transaction (mei-friend's `<application>` stamp is part of what fragments its undo).
   - Ctrl/Cmd-Z in the preview routes to the text model's undo, as VHV and mei-friend do. **There is no separate model undo stack** (unlike Neon and Verovio) [V-src, V-doc #179/#135].
3. **Guard the sync loop explicitly.** Use the equivalent of mei-friend's `blockChanges` / `allowCursorActivity`: while a visual edit is being applied, suppress cursor→preview highlighting and "text changed" handling. Then trigger exactly one compile [V-src].
4. **Hide the 0.3–0.9 s compile with an optimistic preview plus a deferred commit render.**
   - Immediately patch the current SVG: move a notehead by ±½ staff space per diatonic step, toggle a glyph class or colour, or ghost an inserted note. This follows Neon's optimistic transforms and OSMD's advice to mutate SVG nodes for instant feedback.
   - Coalesce bursts: holding ↑ previews each step and compiles once on keyup, as verovio-editor's keyDown → preview / keyUp → `commit` does.
   - Drop superseded compiles (latest-wins), and show a subtle "stale" state as mei-friend does ("Rendering paused…") rather than flashing errors [V-src; I].
5. **Make selection survive edits by mapping source positions, not by trusting ids.**
   - Store the selection as source offsets (Monaco decorations with appropriate stickiness), mapped through every edit as Strudel does with `locations.map(tr.changes)`.
   - After the recompile, re-select the grobs whose `textedit` position equals the mapped offset.
   - Record the selection *before* dispatching, as Neon's comment warns [V-src].
6. **Keep the caret / input model close to MuseScore's "note name" mode and Dorico's caret; LilyPond already speaks it.**
   - Durations on digits. The MuseScore and Dorico layouts differ: MuseScore 5 = quarter, Dorico 6 = quarter. Pick one and make it rebindable.
   - Letters A–G insert `c d e…` with octave "closest to the previous note". **That is literally `\relative`**, so inside `\relative` Lily writes bare letters and adds `'`/`,` only past a fourth.
   - `0` = rest (`r`), `.` = dot, `T` = `~`, Shift+letter = add a chord note (`<…>`), `↑/↓` = semitone, Ctrl+↑/↓ = octave.
   - Use **insert semantics, not overwrite.** LilyPond has no pre-filled measures, which matches Dorico's stream model and MuseScore's Insert method.
   - The caret *is* a text position inside a music expression, shown in the score [V-doc; I].
7. **Dorico-style popovers that accept LilyPond syntax.**
   - `Shift+D`, `Shift+K`, `Shift+M`, `Shift+T`, `Shift+C` (plus a generic "type any post-event / command" popover) open a tiny Monaco input at the selection.
   - Pre-fill it with the existing item; insert the typed text (`\mf`, `\key d \major`, `\clef bass`, `-.`, `\fermata`) verbatim at the right token boundary.
   - Lily's existing completion and diagnostics can run inside it.
   - The mini-language is the source language itself, so nothing needs translating [V-doc for Dorico; I for Lily].
8. **Handle "one token, many grobs" and context effects explicitly.**
   - A note inside a variable used twice, `\repeat unfold`, a chord repetition `q`, or a `\relative` successor whose octave depends on the edited note: these have no analogue in MEI or kern (1:1 mappings).
   - Show the edit scope ("this note appears 3×"; the recent playback commit shows the problem is already known).
   - When changing a pitch inside `\relative`, compensate the octave marks of the next note so the following music does not jump. That is the LilyPond version of mei-friend #114 [I].
9. **Scope v1 like VHV.** Graphically: pitch, accidentals, duration/dots, articulations, dynamics, slurs/ties, delete, and insert-after.
   - Leave lyrics, complex rhythm/tuplet restructuring, multi-voice surgery and layout overrides to the text pane at first.
   - A layout drag that writes `\tweak extra-offset` / `\offset` on the token is a phase-2 item, like mei-friend's Alt-arrows writing `@ho/@vo` [V-doc, V-src].
10. **Navigate by geometry plus voice.** ←/→ next/previous grob in the same voice/staff from the SVG x-order, and ↑/↓ across voices/staves by nearest x then y, as mei-friend's `navigate()` does. Fall back to source order when the geometry is ambiguous [V-src].
11. **For speed on big scores, compile less.** mei-friend's page-excerpt speed mode suggests compiling around the edited region with context. With LilyPond this could be `showFirstLength`/`showLastLength` or `skipTypesetting` [K; feasibility untested]. Only worth it if large scores are a real complaint.
12. **MIDI and pitch spelling.**
    - Verovio spells MIDI input from the key signature and earlier accidentals (`insertCursorByPitch {midi}`). Lily needs the same to choose `fis` vs `ges`.
    - In the Tauri app on macOS (WKWebView), Web MIDI is likely unavailable, so MIDI input would come from the Rust side, for example the `midir` crate [I, not verified].
13. **Code reuse.**
    - Code can be reused only from MIT/BSD projects: abcjs (selection and drag ideas; ABC-specific), Neon (drag/reselect patterns), VexFlow, OSMD.
    - alphaTab is MPL-2.0 (file-level copyleft, usable if modified files stay MPL).
    - mei-friend, verovio-editor and Strudel are AGPL; EasyABC is GPL-2; VHV has no licence. Take ideas only from those.
    - There is no component worth importing; the value is in the patterns above [V-doc licences via the GitHub API and LICENSE files].

### Sources (beyond those inline)
- mei-friend repo <https://github.com/mei-friend/mei-friend>; help <https://mei-friend.github.io/>
- VHV repo <https://github.com/humdrum-tools/verovio-humdrum-viewer>
- Neon <https://github.com/DDMAL/Neon>; verovio-editor <https://github.com/rism-digital/verovio-editor>; Verovio editor toolkit sources <https://github.com/rism-digital/verovio/blob/develop/src/editortoolkit_cmn.cpp>, <https://github.com/rism-digital/verovio/blob/develop/src/editortoolkit_shared.cpp>
- abcjs <https://github.com/paulrosen/abcjs>; click listener <https://docs.abcjs.net/visual/click-listener.html>; editor <https://docs.abcjs.net/interactive/interactive-editor.html>
- EasyABC <https://github.com/jwdj/EasyABC>
- Strudel <https://codeberg.org/uzu/strudel>
- OSMD README <https://unpkg.com/opensheetmusicdisplay@2.0.0/README.md>
- alphaTab LSP <https://alphatab.net/docs/alphatex/lsp>
