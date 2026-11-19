# 03 — Bidirectional text ↔ visual editing outside music

Prior art for Lily's planned visual editor: edits made on the rendered score are written back into
the `.ly` text, and the text stays the source of truth.

**Confidence markers.** **[V]** = verified this session from a primary source (paper text, docs,
source code, or the local `monaco-editor@0.57.0` typings in this repo). **[S]** = from a secondary
source (blog, forum, third-party write-up). **[I]** = my inference or general knowledge, not checked
this session.

---

## 0. The question in one picture

Every system below falls on one axis:

| Where the truth lives | Where a visual edit is computed | Examples |
|---|---|---|
| **Text only**, no second model | A minimal text patch, computed from a parse of the text and applied as an ordinary editor transaction | Overleaf Visual Editor, Obsidian Live Preview, Strudel sliders, Flutter Property Editor, Xcode inspector (apparently), Sketch-n-Sketch |
| **Text, plus a lossless tree that reprints untouched parts verbatim** | An edit to the tree, reprinting only the changed nodes | recast, LibCST, Roslyn, rust-analyzer |
| **Own model**, with text as import/export | Regenerated from the model | LyX, TeXmacs, Typora / Milkdown / RStudio-Quarto visual mode, GitLab Rich Text editor, MPS (projectional) |

The evidence lines up behind the first row, with the second row's machinery underneath it. Systems in
the third row all show the same complaints: reformatting, lost constructs, and raw-code escape hatches.

---

## 1. TeX family

### 1.1 SyncTeX (forward/inverse search only, no edits)

- **Granularity: line, not column.** [V] Laurens (TUGboat 29:3, 2008) writes that SyncTeX records, for
  each hbox/vbox, "the stored file name, the stored line number, the horizontal and vertical positions,
  and the dimensions". It also records glue, kern and math nodes, "because they are created in TeX's
  mouth, when the current input record is still accurate". Boxes are created later, during line
  breaking, and would otherwise all point at the paragraph's last line. Precision is "in general … ±1
  line". On columns: "the original TeX engine does not handle column positions at all", and adding
  them "might need a great amount of work, probably much greater than the eventual benefits".
  https://tug.org/tugboat/tb29-3/tb93laurens.pdf
- **Word precision by text matching.** [V] iTeXMac2 gets word or character precision by sending the
  editor "an extract of text surrounding the character under the mouse" as a hint, and the
  `synctex` CLI has `-h` for that hint (same paper, §9.1).
- **Generated intermediates.** [V] Synchronisation lands in the `.bbl`, not the `.bib`. The paper
  proposes a "SyncTeX map file" from generated lines back to original lines (§5.2).
- **Neutral observer.** [V] Earlier macro-based approaches (srcltx, pdfsync) inserted invisible
  material that "can alter significantly the line breaking mechanism". SyncTeX lives in the engine,
  so "the layout of the documents is exactly the same whether SyncTeX is activated or not".
- **Relevance to Lily.** LilyPond's `-dpoint-and-click` is already better than SyncTeX: it is per
  grob and gives LINE:CHAR:COL. It is also neutral. A tag on the *event* (a notehead's music
  expression) is what you want to edit, whereas TeX can only tell you a line. The lesson is to keep
  the mapping inside the engine's own provenance, not in injected markup. Do not add markup to the
  user's music to get anchors, because it changes the layout.

### 1.2 Overleaf Visual Editor: the closest analogue, and the model to copy

- **No separate model.** [V] Ben Davies (TUGboat 44:2, 2023): "the Visual Editor is now effectively
  the Code Editor with decorations". The previous Rich Text mode, built on CodeMirror 5 with a
  separate Ace-based source editor, could not share track changes or comments, so "people needed to
  switch back and forth between the modes". They later learned that "just because we know how we
  would write the LaTeX doesn't mean we know what the 'button' should do and vice versa".
  https://tug.org/TUGboat/tb44-2/tb137davies-visual.pdf
- **Text is the truth; edits are minimal patches.** [V] Mathias Jakobsen (TUGboat 46:2, 2025): "the
  underlying LaTeX code is still the source of truth. Any changes performed in the WYSIWYG tool must
  be transformed into changes to the LaTeX code, which will in turn update the visual
  representation." For the table generator, which can make structural changes, they follow **three
  guidelines: "Perform minimal edits; Write appropriate LaTeX; and If we don't understand it, we
  don't edit it."** Concretely, whitespace is left "exactly as-is when editing a table", so that
  Tracked Changes shows only real content changes. Where the column definition is not understood,
  they "fall back to displaying the source code rather than guess".
  https://www.tug.org/TUGboat/tb46-2/tb143jakobsen-parsing-editing.pdf
- **A best-effort parser that ignores macros.** [V] Same paper: a Lezer grammar with static
  tokenisation. Category-code changes and macros such as `\def\typesetasmath#1{$#1$}` are knowingly
  misparsed. Specific commands (`\cite`, `\includegraphics`, `\textbf`) get `@specialize` rules.
  Lezer's error recovery matters because "a typo is not going to incorrectly replace a large portion
  of your document with a WYSIWYG widget".
- **Implementation.** [V] Source:
  `services/web/frontend/js/features/source-editor/extensions/visual/atomic-decorations.ts`.
  - A `StateField` walks `syntaxTree(state)` and emits `Decoration.replace` widgets: `MathWidget`,
    `TabularWidget`, `GraphicsWidget` (with `EditableGraphicsWidget` variants), `FootnoteWidget`,
    `IconBraceWidget` for `\ref`/`\cite`, and others.
  - `shouldDecorate` returns `!selectionIntersects(selection, extents)`, so the cursor entering a
    construct reveals its source.
  - While the parse is incomplete (`tree.length < doc.length`), decorations are only *mapped* through
    `tr.changes` ("still parsing"). While the mouse is down, recomputation is skipped.
  - `EditorView.atomicRanges` makes widgets atomic for the cursor.
  - User `\newcommand`/`\def` definitions are collected and passed to MathJax, so custom maths macros
    render.
  - https://github.com/overleaf/overleaf/tree/main/services/web/frontend/js/features/source-editor/extensions/visual
- **Ambiguity from macros.** [I] Overleaf largely avoids the problem: it decorates *the source in
  place*, so every widget corresponds to one source range. Macro uses are shown as their call site.
  The compiled PDF is not the editing surface, and that is the key difference from Lily (§9).
- **Complaints.** [S] Constructs it does not recognise stay raw. Pasted tables cannot "fully preserve
  the form and style". Reveal-on-cursor makes layout jump (see Obsidian below).

### 1.3 LyX: own model, LaTeX export, and ERT boxes

- [S] tex2lyx: the output is "not guaranteed to create a LyX file which generates exactly the same
  output", and the tool will "err on the side of translating less", leaving more as ERT (Evil Red
  Text, i.e. raw TeX boxes). LyX's own test suite labels some ERT/preamble round trips as known
  failures ("ertroundtrip"). https://dyn.manpages.debian.org/jessie/lyx/tex2lyx.1.en.html ·
  https://code.ascee.nl/anne/lyx_mirror/commit/c44cfec3486028d942f6a710c7ce133c3378a0f3
- **Lesson.** [I] A model-first editor needs an "opaque raw code" node for everything it does not
  model. In a macro-heavy language that node grows to swallow most real documents. Lily's equivalent
  would be Scheme and user music functions. Text-first avoids this, because the raw code is simply
  the text.

### 1.4 TeXmacs: own model, lossy converters

- [V] The TeXmacs manual: TeXmacs→LaTeX primitives without a counterpart "will usually be dropped or
  approximated". LaTeX→TeXmacs reliably converts only a subset, cannot handle style files, and is
  confused by catcodes or unusual macro definitions. Unknown macros appear in red.
  https://www.texmacs.org/tmdoc/main/convert/latex/man-problems.en.html

---

## 2. Typst (typst.app web editor, tinymist)

- **Spans are node IDs, not byte ranges.** [V] "Each syntax tree node gets a unique number." Numbers
  "stay mostly stable, even for nodes behind an insertion". They are resolved to a byte range on
  demand (`WorldExt::range`). https://docs.rs/typst-syntax/latest/typst_syntax/struct.Span.html
  This is Typst's answer to stale positions: an output glyph refers to a node ID, and the ID survives
  edits elsewhere in the file.
- **Click → source.** [V] `typst_ide::jump_from_click`:
  - Each glyph carries `(span, span_offset)`.
  - On a text node the jump goes to node start + offset, rounding to the next character past the
    glyph's midpoint.
  - Shapes and images go to `node.offset()`.
  - A *detached* span is skipped.
  - Spans in other files, including imported templates, resolve into those files. Nothing restricts
    the jump to the main file.
  - `jump_from_cursor` returns *one position per matching page*: the same span can appear many times.
  - https://raw.githubusercontent.com/typst/typst/main/crates/typst-ide/src/jump.rs ·
    https://docs.rs/typst-ide/latest/typst_ide/
- **Ambiguity.** [V/I] A click on text produced inside a function goes to the span where that text
  was *written*, i.e. into the function or template body, not the call site. Typst does not resolve
  this ambiguity; it reports the origin. LilyPond's textedit links behave the same way: a note inside
  `melody = { … }` links into the definition.
- **tinymist preview.** [S] Bidirectional jump and `tinymist.preview.scrollSync`
  (`never` / `onSelectionChangeByMouse` / `onSelectionChange`). There is an experimental
  `cursorIndicator`.
  https://git.joshthomas.dev/language-servers/tinymist/src/commit/25bb1b2fbc68fd6034a85ca9a3dca14b92584eb8/editors/vscode/Configuration.md
  - Users complain about the sync firing on every click (they want a modifier key) and about jumps
    breaking after a page background was added.
  - https://forum.typst.app/t/how-to-set-the-synctex-jump-key-to-ctrl-left-click/3733 ·
    https://forum.typst.app/t/tinymist-why-doesnt-jump-from-preview-to-code-work-after-adding-a-page-background/2145
- **Edit-from-preview.** [S, negative result] I found no evidence that either typst.app or tinymist
  writes edits from the preview back into source. They stop at navigation. Incremental compilation
  makes their preview fast enough that "edit the text, see it" is the loop.

---

## 3. Markdown live preview (widgets replacing source ranges)

- **Obsidian Live Preview.** [S] A state field supplies `Decoration.replace` widgets "when the
  cursor is not overlapping with the target positions" (the same pattern as Overleaf).
  https://docs.obsidian.md/Plugins/Editor/Decorations ·
  https://forum.obsidian.md/t/plugin-api-expose-live-edit-functionality-for-extension/73447
  - **Complaints:** lines shift and the cursor jumps as syntax hides and reveals.
    https://forum.obsidian.md/t/to-hide-markdown-syntax-in-live-preview-so-it-doesn-t-show-formatting-markers/109155
  - Replace widgets are hard to make editable in their own right.
    https://forum.obsidian.md/t/how-to-create-custom-codemirror-block-widget/36132
- **Model-based visual editors and their complaints.**
  - **Typora:** [S] Users report rewritten nested-list indentation and automatic quote conversion
    (https://github.com/typora/typora-issues/issues/750).
  - **RStudio/Quarto visual mode:** [V] Quarto added an `editor: markdown: canonical: true` option so
    that *source* mode writes the same canonical markdown as visual mode, keeping version-control
    diffs clean. In other words, they gave up on preserving the user's formatting and made everybody
    canonical instead. https://quarto.org/docs/visual-editor/options
    - Complaint: https://forum.posit.co/t/is-there-any-way-to-stop-visual-mode-from-reformatting-my-markdown/118608
  - **Milkdown:** [I] ProseMirror ↔ remark AST. Anything the mdast does not represent is normalised.
- **GitLab Rich Text editor: the cautionary tale.**
  - [V] The editor "re-generates the entire Markdown source", so a one-word edit rewrites the whole
    document, for example turning setext headings into ATX.
    https://gitlab.com/groups/gitlab-org/-/epics/7256
  - The fix ("retain original markdown for all blocks that were not touched by the user") shipped
    behind `preserve_unchanged_markdown`.
    https://gitlab.com/gitlab-org/gitlab/-/merge_requests/68702
  - A later sourcepos-based `preserve_markdown` flag was **removed**, citing "blockers requiring
    somewhat uncapped R&D to solve (sourcepos fidelity)" and "lack of a clear value proposition".
    https://gitlab.com/gitlab-org/gitlab/-/issues/584484
  - **Lesson.** Bolting source preservation onto a regenerate-from-model editor, through positions
    reported by a separate renderer, is a research project. Build text-first from the start.

---

## 4. UI code with a live canvas

### 4.1 Xcode SwiftUI Previews

- **Canvas and inspector write code.** [V] WWDC20 "Visually edit SwiftUI views": the inspector "can
  add and edit common modifiers … This immediately updates your code and renders in the canvas".
  Structural actions (Embed in HStack, Repeat, Duplicate) are offered from ⌘-click *in the code
  editor*. https://developer.apple.com/videos/play/wwdc2020/10185/
- **Rolled back.** [S] Drag-and-drop *into the canvas* was removed in Xcode 15, per an Apple forum
  reply. https://developer.apple.com/forums/thread/738056 ·
  https://developer.apple.com/forums/thread/746483
  - [I] Apple moved toward "canvas for viewing and selecting, code and inspector for editing".
- **Latency trick: hot-swapped literals.** [S] Xcode rewrites literals in the preview build as
  `__designTimeString("<id>", fallback: "…")` (and `Integer`/`Float` variants). Each ID keys a value
  dictionary, so "minor tweaks do not require any code compilation". Other body edits go through
  `@_dynamicReplacement`. Signature changes need a full rebuild, which pauses previews.
  https://www.guardsquare.com/blog/behind-swiftui-previews · https://fatbobman.com/en/posts/how-swiftui-preview-works
  - Users complain about previews pausing on large edits and about the instrumentation breaking code
    (e.g. `__designTimeString` with OSLog under Swift 6).
    https://developer.apple.com/forums/thread/778502
- **Mechanism of code rewriting.** [I] Apple has not documented it. It is presumably SwiftSyntax/
  SourceKit-level edits to the modifier chain. I could not verify this.

### 4.2 Android Studio Compose

- [S] **Live Literals**, the predecessor, let you change "small individual values" in real time, the
  same trick as Xcode's. **Live Edit** extends this to function bodies but not to signature or class
  hierarchy changes. When compilation fails it shows "Out Of Date".
  https://developer.android.com/develop/ui/compose/tooling/iterative-development (via search) ·
  https://levelup.gitconnected.com/jetpack-compose-live-edit-vs-re-run-when-real-time-updates-hit-a-wall-3518ea422ccd
- Compose's preview does not offer canvas → code editing beyond literals.

### 4.3 Flutter Property Editor (3.32+)

- **Cursor-driven, literal-only.** [V]
  - "Move your cursor anywhere inside the widget constructor invocation." The panel shows the
    properties of the widget at the cursor.
  - string, int and double fields: "Press Tab or Enter to apply the edit directly to your source
    code". bool and enum: dropdowns.
  - **Object-valued properties (`TextStyle`, `EdgeInsets`, `Color`) are "Currently not supported …
    edit these directly in your source code."**
  - Clicking a widget in the running app (Select Widget Mode) jumps to its source and loads it into
    the panel.
  - https://docs.flutter.dev/tools/property-editor
- [I] Flutter knows each widget's creation location because `--track-widget-creation` rewrites
  constructor calls to carry it. This is the same kind of "call-site provenance" as LilyPond's
  textedit links.

### 4.4 Figma ↔ code

- [I] Round trips do not exist in practice. Code Connect and Dev Mode are one-way: design to code
  snippets. Nobody keeps a canvas edit and hand-written code in sync. Mentioned only to rule it out.

---

## 5. Programming by direct manipulation (research)

### 5.1 Sketch-n-Sketch (Chugh et al.)

- **Trace-based synthesis (PLDI'16).** [V] https://ar5iv.labs.arxiv.org/html/1507.02988
  - Each output number carries a trace, for example `155 = (+ x0 (* (+ ℓ1 (+ ℓ1 ℓ0)) sep))`. A drag
    becomes an equation, and only **numeric literals** may change. One location is chosen per
    attribute and solved univariately.
  - **Ambiguity.** In the 68 examples, **59% of active zones were ambiguous (3.83 candidates on
    average)**. It is resolved by:
    - **freeze** annotations (`!`): all Prelude constants are frozen;
    - **range annotations** that turn a literal into a slider;
    - a **"fair" heuristic**: give each shape a location set not yet given to another shape, then
      rotate;
    - a "biased" heuristic: prefer locations that appear in fewer traces.
  - **Shared constants** propagate to every use: dragging one Ferris-wheel car widens all of them.
    They also mis-fire: a drag changed `numSpokes`. The author's fix was to freeze those constants
    and give them sliders.
- **Latency.** [V] Solve takes under 1 ms and re-evaluation about 12 ms per drag frame. Parse
  (77 ms) and "Prepare" (200 ms average, 6.8 s maximum) run only before or after a drag, not during
  it. So: cheap re-evaluation per frame, and expensive analysis only at mouse-up.
- **Bidirectional evaluation (OOPSLA'18, Mayer/Kunčak/Chugh).** [V] https://ar5iv.arxiv.org/html/1809.04209
  - When several program repairs fit, the editor shows them as candidates: the user hovers each to
    preview its code and output, then picks one.
  - **Two-way merge** requires every occurrence of a variable to change consistently, or the update
    fails.
  - **Three-way merge**, the default, "succeeds despite conflicts", so the user can change one use of
    a shared variable (`let x = 1 in [x, x]` → `[1, 2]` gives `[2, 2]` under three-way merge; two-way
    fails).
  - `freeze` protects expressions.
  - Synthesis takes 0–2 s. Auto Sync waits 1000 ms.
  - Formatting preservation is not discussed.
- **Output-directed programming (UIST'19).** [V] https://ar5iv.labs.arxiv.org/html/1907.10699
  - Provenance tags on every value. Selections in the output feed a tools menu that offers *several
    candidate programs, each previewed (canvas + code diff) on hover*.
  - Make Equal "prefers changes that rewrite terms near each other and later in the program".
  - A function used many times is handled by **focusing** a call, which shows that call's arguments
    and inserts drawings into the function's body. Focus markers are stored as special comments.
  - Stated limits: Make Equal "can offer many hard-to-distinguish options"; it "becomes sluggish on
    larger examples" because of trace comparison. There was no user study.

### 5.2 Bret Victor

- [S] *Inventing on Principle* (2012): creators need an "immediate connection"; scrub a number in
  code and see the output change. https://jamesclear.com/great-speeches/inventing-on-principle-by-bret-victor
- [I] The practical descendants are literal scrubbers: Inlet for CodeMirror
  (https://github.com/enjalot/Inlet), Strudel (§6), and Xcode/Compose live literals (§4).

### 5.3 Projectional and structure editors

- **Hazel.** [S] Typed holes give "every editor state" a static and dynamic meaning. Livelits fill
  holes with live GUIs. https://ar5iv.labs.arxiv.org/html/1805.00155 ·
  https://icfp19.sigplan.org/details/tyde-2019-papers/12/Livelits-Filling-Typed-Holes-with-Live-GUIs
- **JetBrains MPS.** [S] The research literature calls usability "the main drawback of projectional
  editors": entering text, selecting and modifying code, and inconsistent per-language interaction.
  "Grammar cells" were invented to make it feel like a text editor again.
  https://www.pl.informatik.uni-mainz.de/files/2019/04/grammar-cells.pdf ·
  https://gsd.uwaterloo.ca/projectional-workbenches.html
- **Lesson.** [I] Do not make the visual editor a projectional editor of `.ly`. Users of a
  programming language want the text to stay freely editable.

### 5.4 Bidirectional transformations and lenses (Boomerang)

- [S] Lenses have `get` (source → view) and `put` (edited view + old source → new source). The hard
  case is **ordered data**: reorder the view and the chunks of source behind it fall out of alignment.
  Boomerang's *dictionary lenses* mark "reorderable chunks" and **keys** so that `put` re-matches
  view items to their source chunks by key, not by position ("resourcefulness").
  https://repository.upenn.edu/cis_reports/757 ·
  https://www.inf.ed.ac.uk/teaching/courses/apl/2010-2011/slides/apl16.pdf
- **Lesson.** [I] Lily's key is the textedit href. When notes are reordered, inserted or deleted,
  identify each output element by its href, not by its index in the SVG. Lily already does this for
  playback (D26/D118/D120).

---

## 6. Parametric CAD and creative coding

- **OpenSCAD Customizer.** [V/S] Comment annotations (`//[0:2:50]`) on top-level assignments make
  sliders. The Customizer's values are **"just temporary overrides to the defaults"**: saved parameter
  sets go to a JSON file beside the `.scad`, and the source is not edited.
  https://files.openscad.org/documentation/manual/Customizer.html ·
  https://lists.openscad.org/empathy/thread/SSREPJYDN5OT6CKWA4BJE5PL3BHEQS6D
  This is a deliberate dodge: write-back is avoided by keeping overrides out of band.
- **CadQuery / build123d (OCP CAD Viewer).** [S] A viewer with an object tree, fed over a websocket.
  I found no write-back or click-to-source. https://github.com/bernhard-42/vscode-ocp-cad-viewer
- **Strudel inline sliders: the best small-scale write-back pattern found.** [V] Source:
  https://codeberg.org/uzu/strudel/raw/branch/main/packages/codemirror/slider.mjs
  1. `slider(value, min, max)` in code becomes a CodeMirror widget that remembers `from`/`to`. On
     every transaction the plugin does `this.decorations = this.decorations.map(tr.changes)` and
     copies the mapped `from` back into the widget. **Positions are kept fresh by mapping through
     changes, not by re-parsing.**
  2. On input it dispatches `{ changes: { from, to: from + originalValue.length, insert } }`, an
     ordinary CodeMirror change to the literal's exact range.
  3. **At the same time** it calls `window.postMessage({type:'cm-slider', value, id})`. The running
     pattern reads `sliderValues[id]` lazily through `ref(() => sliderValues[id])`. **The audio
     changes without re-evaluating the program.** The ID is derived from the literal's original
     position, which the transpiler also uses. Re-evaluation resets the value from code.
  - So Strudel has two paths: a **fast path**, a side channel keyed by source position that bypasses
    the compiler, and a **truth path**, the text edit. They converge on the next evaluation.
- **Hydra / Shadertoy-style widgets.** [I] Same family (uniforms tweaked live, code re-run). Not
  investigated further.

---

## 7. Formatting-preserving rewriting machinery

- **Roslyn.** [V] Trees are "full fidelity" and "completely round-trippable back to the text";
  whitespace, comments and directives are trivia attached to tokens. Trees are immutable, so a
  modification makes a new snapshot through factory or `With*` methods.
  https://learn.microsoft.com/dotnet/csharp/roslyn-sdk/work-with-syntax ·
  https://github.com/dotnet/roslyn/blob/main/docs/wiki/Roslyn-Overview.md
- **rust-analyzer / rowan.** [V] "Syntax trees are lossless … All comments and whitespace get
  preserved".
  - The parser is resilient: `ERROR` nodes are kept, and missing nodes are simply absent.
  - Immutable, interned *green* nodes sit under positioned *red* cursors.
  - Incremental reparse re-parses only the enclosing `{}` block, though "parsing from scratch is
    usually fast enough".
  - https://github.com/rust-lang/rust-analyzer/blob/master/docs/book/src/contributing/syntax.md
- **tree-sitter.** [S] Call `tree.edit(InputEdit)` with old and new byte and point ranges, then
  `parse(newText, oldTree)`, which reuses unchanged subtrees. Node handles held across an edit go
  stale and must be re-fetched (https://debbugs.gnu.org/db/60/60656.html).
  [I] A community `tree-sitter-lilypond` grammar exists. Its quality has not been checked.
- **recast.** [V] "reprints only those parts of the syntax tree that you modify". It keeps a shadow
  copy with `.original` links and falls back to pretty-printing only the modified node.
  https://github.com/benjamn/recast
- **LibCST.** [V] "a lossless CST that looks and feels like an AST", which "preserves all whitespace
  and can be reprinted exactly". https://libcst.readthedocs.io/en/latest/why_libcst.html
- **Editor transactions and undo.**
  - **Monaco** [V, local `node_modules/monaco-editor/monaco.d.ts`, v0.57.0]:
    - `ICodeEditor.executeEdits(source, edits, endCursorState)`: "The edits will land on the
      undo-redo stack, but no 'undo stop' will be pushed."
    - `pushUndoStop()` / `popUndoStop()` control grouping.
    - `ITextModel.pushEditOperations` is "the preferred way". `applyEdits` edits "without adding the
      edits to the undo stack. This can have dire consequences on the undo stack!"
    - Decorations are tracked ranges (`TrackedRangeStickiness`) that move with edits.
    - Recipe: one visual action = `pushUndoStop(); executeEdits('lily-visual', edits, selections);
      pushUndoStop()`. Never `setValue`, which wipes undo and cursor.
  - **CodeMirror 6** [V]:
    - `ChangeDesc.mapPos(pos, assoc, mode)` with `MapMode.TrackDel`, which returns null if a deletion
      crossed the position. This is exactly the "is my stale target still there?" test.
    - `Transaction.addToHistory` and the `userEvent` annotation. [I] `isolateHistory` stops merging
      with adjacent events.
    - https://codemirror.net/docs/ref/
  - **The VS Code extension** [I]: `WorkspaceEdit` through `workspace.applyEdit`, or
    `TextEditor.edit(builder, {undoStopBefore, undoStopAfter})`, which follows the same grouping
    model.
- **Collaboration.** [V] Yjs explains why index positions break: "the index-range is invalidated as
  soon as a remote change manipulates the document". *Relative positions* are anchored to an element
  and "always point to the same location". https://docs.yjs.dev/api/relative-positions
  - [V] Overleaf's minimal-edit guideline exists partly because Tracked Changes and comments sit on
    the same text. Reformatting would light up the whole document.

---

## 8. Cross-cutting answers to the five questions

| Question | What the field does |
|---|---|
| **Where is the edit computed?** | Successful systems compute a *minimal text patch* against a parse of the current text and apply it as an ordinary editor transaction: Overleaf, Strudel, Flutter, Obsidian, and Sketch-n-Sketch (literal substitution). Regenerate-from-model systems (Typora, Quarto, GitLab, LyX, TeXmacs) are where the reformatting and lost-construct complaints concentrate; GitLab abandoned a fix after it ran into sourcepos fidelity problems. |
| **One output element from code used many times** | Provenance points at the **definition/origin** (Typst spans, Sketch-n-Sketch traces, LilyPond textedit). Responses: (a) change the shared literal and let *all* uses change, which is honest and the default in SnS's trace synthesis; (b) let the user choose among candidates with hover previews (SnS OOPSLA'18/UIST'19); (c) **freeze** things that must not be touched (SnS `!`, Prelude); (d) **focus** a call site; (e) three-way merge to "change just this use" (needs a language-level split); (f) restrict editable things to *literals at the cursor* (Flutter, Xcode inspector). No system silently guesses for non-literals. |
| **Stale positions while re-render is pending** | (1) Map stored positions through every change since the render (CodeMirror `mapPos`/`decorations.map(tr.changes)`, Monaco tracked decorations); (2) give positions identity rather than offsets (Typst span numbers, Yjs relative positions); (3) treat a mapped-through-deletion target as dead (`MapMode.TrackDel`); (4) while parsing is incomplete, map the old decorations and do not recompute (Overleaf). |
| **Latency tricks** | (1) **Side-channel literal hot-swap** keyed by source position, bypassing the compiler, with the text edit as the truth path (Strudel, Xcode `__designTime*`, Compose Live Literals); (2) cheap per-frame evaluation during a drag, heavy analysis only at mouse-up (SnS); (3) debounce (SnS Auto Sync 1000 ms); (4) degrade gracefully, e.g. "paused" / "Out Of Date" indicators instead of a wrong preview (Xcode, Compose). |
| **Undo** | One stack, owned by the text editor. Each visual action is one grouped transaction (Monaco undo stops / CM6 history). Overleaf gets undo, track changes and comments for free precisely because the visual editor *is* the code editor. |
| **Complaints** | Reformatting and noisy diffs (GitLab, RStudio, Typora); layout jump on cursor reveal (Obsidian); unsupported constructs falling back to raw code (LyX ERT, Overleaf tables, Flutter object properties); too many indistinguishable candidates (SnS); heuristics changing the "wrong" shared constant (SnS `numSpokes`); clicks syncing when not wanted (tinymist); the canvas editing surface being withdrawn (Xcode 15 drag-and-drop). |

---

## 9. Where Lily differs

- **The editing surface is the *compiled output*, not decorated source.** This is unlike
  Overleaf and Obsidian and like Xcode, Flutter and Sketch-n-Sketch. The grob → `textedit://` link
  gives *the origin token*, but nothing about which use of a variable or which iteration of
  `\repeat` produced a given notehead. Lily already reconstructs "which use" for playback from
  geometry and timing (D118, D120).
- **Compile latency is 0.3–0.9 s**, with no runtime to hot-swap values into. There is no Strudel or
  Xcode-style side channel *into lilypond*. The only fast path is a local prediction drawn on the SVG.
- **The language is macro-heavy** (music functions, Scheme). Like TeX, it cannot be parsed exactly
  without running it, so Overleaf's best-effort parser plus its "if we don't understand it, we don't
  edit it" rule is the closest fit.

---

## Lessons for Lily (ranked)

1. **Text only, with visual edits as minimal text patches. No second model.**
   - Adopt Overleaf's three rules verbatim as the design contract: *perform minimal edits; write
     idiomatic LilyPond; if we don't understand it, we don't edit it*.
   - A visual action resolves to `(href → source range → syntax node) + intent → TextEdit[]`. The
     edits touch only the tokens they must, and never re-print a music expression.
   - The model-based path (LyX, TeXmacs, Typora, GitLab, Quarto canonical mode) predictably ends in
     reformatting complaints and raw-code islands. GitLab spent years and then *removed* its attempt
     to retrofit preservation. **[V basis, I conclusion]**

2. **Recommended architecture for a macro-heavy text language: anchor, analyse, patch, reconcile.**
   - **Anchor.** The textedit href gives LINE:CHAR:COL of the *origin token*. Resolve it against the
     **text version that was compiled**, not the current one (see 4), then into a node of a
     **lossless, error-tolerant, incremental** parse of `.ly`, in the style of tree-sitter or rowan,
     with whitespace and comments as trivia.
     - Scheme forms (`#(…)`, `$…`) and unknown music-function bodies become **opaque nodes**. This is
       the ERT idea, but they stay plain text.
   - **Analyse.** Classify the anchor:
     - (a) a literal at a use site (pitch, duration, articulation, dynamic in the main music): edit in
       place;
     - (b) a token inside a variable/function definition referenced N > 1 times: **shared**;
     - (c) inside an opaque or unknown construct, or in a file outside the project, or generated
       (`\include`d library, `ly/` init files): **frozen**. Show "edit in code" and jump there, never
       guess.
   - **Patch.** Each tool is a small function producing `TextEdit[]` on the node, e.g. change pitch
     token, change duration, add or remove a postfix script, insert an event after a node. Compute it
     on the parse of the *current* text and splice whole tokens. Formatting elsewhere is untouched by
     construction. This is recast/LibCST behaviour without needing a general printer.
   - **Reconcile.** Apply through the editor's normal transaction API. Recompile. The new SVG
     replaces the provisional one.
   - **The Rust port in Studio.** Keep the patch functions in `@lily/common` (no `vscode`, no Node)
     so that both apps share one implementation. The extension applies them through `WorkspaceEdit`,
     Studio through Monaco `executeEdits`. **[I]**

3. **Make shared-code ambiguity explicit; never guess silently.**
   - Default: edit the origin token, as Typst, SyncTeX and SnS trace-synthesis all do. **Before
     committing, highlight every other rendered place with the same href.** Lily already computes
     these places (`sourceLinks`, D118/D120). Label the action "changes all 4 uses of `\melody`".
   - Offer alternatives only where an idiomatic text form exists, previewed on hover as in SnS:
     - "only this one" via `\tweak`/`\once \override` at the *use* site, for layout properties;
     - for a pitch or duration, unfolding is a much bigger edit, so say so and send the user to the
       code.
   - Add a **freeze** concept: the user's own includes and library files are read-only targets by
     default.
   - SnS data warns against heuristics alone: 59% of drag targets were ambiguous, and heuristics
     picked surprising constants. **[V data, I recommendation]**

4. **Stale positions: stamp, map, and refuse.**
   - Stamp every render with the editor's model version (Monaco `getVersionId()`, VS Code
     `document.version`). Keep the change log since that version, or simpler, turn the render's
     href targets into **Monaco tracked decorations / CM-style mapped ranges** at render time, so
     that they move with typing exactly like Strudel's widgets.
   - When a visual edit fires, map its target from the render's version to now. If the mapping
     crosses a deletion (`MapMode.TrackDel` semantics, or a decoration whose range collapsed),
     **refuse the edit and wait for the next render** rather than patch the wrong token.
   - Re-validate before splicing: the token at the mapped range must still parse as the expected
     node kind and text. This is cheap and catches races with typing, formatting or external file
     changes.
   - Allow at most one *unreconciled* visual edit per target node. Queue or coalesce the rest.
     **[V mechanisms, I design]**

5. **Optimistic update: a provisional layer over the last real render, replaced wholesale on arrival.**
   - The text edit is applied **immediately**: the editor is truth, and undo works at once.
   - On the SVG, draw a **provisional overlay** for what can be predicted cheaply and honestly:
     - move the dragged notehead by staff positions;
     - ghost an inserted note;
     - fade a deleted one;
     - add a script glyph at an approximate offset.
   - Mark the overlay as provisional (dimmed or outlined). Do *not* attempt local re-layout:
     spacing, beaming and line breaks are lilypond's job.
   - Compile with **latest-wins**: cancel or ignore a compile whose version is older than the newest
     requested, the same rule as the existing preview.
   - When the SVG for version ≥ edit version arrives, drop the overlay and swap in the real render.
     If compilation fails, keep the last good SVG and show an "out of date" state, as Compose and
     Xcode do. Undo is unaffected because the text already holds the edit.
   - For **continuous drags** (pitch scrubbing, nudging a tweak's offset), follow SnS and Strudel:
     - per frame, update only the overlay, with throttled text writes coalesced into one undo step,
       or write the text only on mouse-up;
     - compile on pause or release, not per frame.
   - There is no lilypond equivalent of `__designTimeString`, so do not build one.
   - **[V patterns, I design]**

6. **Undo: one stack, owned by the text editor.**
   - Every visual action is exactly one undo step:
     `pushUndoStop(); executeEdits('lily-visual', edits, selectionsAfter); pushUndoStop()` in Studio;
     a `WorkspaceEdit` or `editor.edit` with undo stops in VS Code.
   - A drag gesture is one step, regardless of how many intermediate writes it made.
   - Never `setValue`/`applyEdits`.
   - Put the cursor and selection on the edited token afterwards, so that ⌘Z and the text view both
     show what changed. Undo from the score is then just the editor's undo.
   - This is what made Overleaf's CM6 rewrite worthwhile: undo, track changes and comments come for
     free. **[V]**

7. **Selection ↔ cursor ↔ inspector, Flutter/Xcode-style, before free-form dragging.**
   - Clicking a grob moves the cursor (already done). A **property panel** follows the *cursor's
     syntax node* and edits only **literal** arguments: duration, pitch, a `\tweak` value, a dynamic.
   - Non-literal arguments are shown read-only with "edit in code", as Flutter does with
     `TextStyle`/`Color`.
   - Xcode later withdrew canvas drag-and-drop. Selection plus inspector is the robust core;
     direct dragging comes after. **[V Flutter, S Xcode, I recommendation]**

8. **Reorder and insert by key, not by index.** For multi-element operations (transpose a selection,
   reorder, delete a range), identify output elements by href (the Boomerang "keys" idea), map keys
   to source nodes, and compute the patch in *source* order. The SVG's element order is not reading
   order (D118). **[I]**

9. **Avoid reveal/hide churn in the text editor.** If Lily ever adds widgets *inside* Monaco
   (inline slider, pitch chip), follow Obsidian and Overleaf: reveal the source when the cursor
   enters. Expect complaints about layout jumping, so prefer widgets that are zero-width or
   end-of-line (Strudel's pattern) over ones that replace ranges. **[S]**

10. **Do not go projectional.** Keep the text editor fully free. Hazel and MPS show the usability
    cost; Lily's users write code, and the visual editor is an accelerator over it. **[S]**
