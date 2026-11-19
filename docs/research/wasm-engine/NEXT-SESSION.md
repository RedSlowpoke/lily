# Next session: see the runway's ledger

> **Superseded as the pointer (D200, 2026-10-18).** The next session
> starts from [`runway/LEDGER.md`](runway/LEDGER.md); what is left is
> planned in [`runway/`](runway/README.md). This file was last kept at MS3
> step 25. Its *Set-up for any session on this branch* (the commands, the
> tablet, the emulator, the Simulator and desktop recipes) is still the
> reference the runway cites; the rest is history.

## MS3, part 3 (as of step 25)

MS3 runs in five parts (progress.md, *MS3*, *The plan*). **Part 1 is
closed** (*MS3 · step 10*), **part 2 is closed** (*MS3 · step 23*) and
**part 3 is split** (*MS3 · step 24*), into steps 25–44, of which step 25
is done. This file is the checklist for the next working session: step
26.
The details are in [`progress.md`](progress.md).

## Where things stand

- Worktree `.worktrees/engine`, branch `feat/engine`. Nothing is pushed.
- **MS0, MS1 and MS2 are closed; MS3's parts 1 and 2 are closed, and
  part 3 is split.** Part 1
  (steps 1–10) built `crates/layout`'s stencils, font metrics over W1's
  packs, font selection and markup's text, and `crates/svg` with W3's
  writer. Part 2 (steps 11–23) split itself (step 11), captured
  LilyPond's grobs of the 10 as references (step 12, made byte for byte
  run to run by step 12a, D109), kept the grob in `crates/layout` beside
  the music crate's record (D108; steps 13 and 13a), gave it LilyPond's
  Scheme face, items and grob arrays (step 14; step 14a's scaled layout
  parented to the book's paper, D110), and engraved the 34 grob types
  with 35 of `CLASSES`' 36 placeholders shadowed by layout's classes and
  `Mark_tracking_translator` in the music crate (steps 15–22).
- **Part 2's gate passed with nothing skipped** (*MS3 · step 23*), in
  two whole runs, both counted: **415 Rust and 145 tool tests**, none
  failed or skipped (500 s and 280 s). **The 10's 1,913 lines (1,903
  grobs of 34 types and 10 Systems) equal step 12's `ref/<id>/grobs.tsv`
  line for line**, natively and in wasm (2,485 with nothing forbidding a
  break, 1,921 with `Bar_engraver` forbidding); **the 29 traces of
  `TRACED`** equal, `TRANSLATED` holding `Tuplet_engraver`'s
  `currentTupletDescription` alone, no `TRANSLATED_IN`; steps 13 and
  14's grob probes (75 records natively, 297 from the image natively and
  in wasm). Part 1's gate unchanged (the 11 pages PAE ≤ 1, links, sizes,
  B10; 267 of 275 markups, the 8 of `EMMENTALER_TEXT`), and MS2's (C1 =
  1.0 on the 30, C0, `syntax.ly`, the worker in Node 4 of 4, b1–b8, the
  function probe 153 of 159).
- **Counted:** the music crate's `CLASSES` 128 with **117 placeholders**,
  **35 shadowed** by layout's 37 classes (`Score_engraver` and
  `Tie_engraver` shadow none), 82 left; **`PRIMS` 221**. The fixture 137
  of LilyPond's 140: `UNMADE`'s three lines are `Script_column_engraver`'s
  and `Fingering_column_engraver`'s, **part 4's** (*The shares*' row 4).
- **T15's `stderr`:** its two `NoteHead` lines LilyPond's; its
  `(process-acknowledged)` lines **18 of 18** at LilyPond's places; its
  `saw end of #<Grob Beam >` line missing, MS4's (`Beam_engraver`); the
  rest MS2 step 26's stream-event printing. 42 of LilyPond's 43 lines in
  order once events are cut.
- **Sizes against step 10:** `core.image` 451,235 → **456,765 brotli
  (1.088 of Must)**; the driver row 99,577 → **103,440 raw (1.293 raw,
  1.418 brotli of Must)**, the core's functions 43,463 → 45,366 and the
  rest 56,114 → 58,074; layout 60,122 → **187,879 raw** (≈ 68,285
  brotli, 207 → 732 functions, 0.171 of Must); SVG 40,208 → 41,182 raw
  (≈ 16,241, within Goal); the data section 207,546 → 249,739 bytes; the
  core's download 1,008,986 brotli, 8,986 over Goal (a budget). **R2:
  no code row and not the image over twice its Must**, nothing to
  report. The music row's −225, the driver's +154 and N7 melisma's −154
  are wasm-opt -Oz merging functions across crates, no music code
  changed.
- **Devices** (desktop Chromium 147 and WebKit 26.4, the iOS 26.3
  Simulator): the translation of the 29 traced **×2.61–2.66 step 10's**
  as part 2's engravers join it (T07 18.0 / 16 / 17 ms, T29 156.3 / 141 /
  159), every score within Must, T12, T29 and T24 over Goal; ≈ 23 µs a
  grob made in desktop wasm. The SVG phase unchanged (T05 1.4 / 1.1 /
  1.2 ms). **The Android tablet could not be reached** (it could not be
  woken); the Android emulator ran in its place at the user's word:
  translation ×2.93 against MS2 step 42's emulator, within Must, SVG
  within Goal, the side figures unmoved.
- **D88: the three tablet misses are measured** (*MS3 · Android tablet, by
  hand*; Silk 152.4.10, step 25's build). The translation of **T12 166.9
  ms (2.78 of Must), T29 685.1 (2.74) and T24 629.6 (2.52)**, within
  −6 % to +6 % of step 23's two projections; T07 81.3 (1.36), under
  twice. Their lead: the translation's cost per grob made. They already
  have their step: step 36, right after the layout path joins the core.
  **One more over twice:** the first `lily_start` in a fresh worker,
  **102.9 ms over 24 workers (2.06 of 50)**, step 10's 76.1 (1.52), with
  the image and restore code unchanged: the tablet's fresh workers run
  fast (49–77 ms) or slow (85–126), and this time 5 of 24 ran fast where
  step 10 had 5 of 9. Reported to the user, no step given yet.
  **R1's ratio:** the tablet runs the 29 traced at **2.92 times** step
  23's emulator (2,984.1 ms against 1,020.8), within the plan's 2.8–3.1.
- **Part 3 is split** (*MS3 · step 24*; its sessions' notes in
  `$LILY_CORPUS/device-runs/2026-10-14-ms3-step24/`). The user's five
  answers (*The plan*, *Settled with the user at part 3's split*): **(1)**
  D88's step 36 profiles the translation and makes it cheaper; **(2)**
  when the tablet cannot be reached, the Android emulator decides R1
  (under 120 µs met, over 120 stop), with the tablet's ratio to it
  (2.8–3.1 at step 23) beside it; **(3)** 05 §5's machinery (typed slots,
  generated tables, Rust in place of Scheme callbacks, a skyline cache
  per glyph, an index of placed skylines) may be built in part 3, each
  piece recorded with its lines; **(4)** layout depends on
  `lily_engine_skyline` from part 3 (D111); **(5)** T23's incipits stop at
  a callback of `PRIMS`' 73, as D100. Its counts (52 checks, all agree)
  are kept in `$LILY_CORPUS/ms3-part3-split/`.
- **Lines (R12):** part 2's source **13,389 added** (steps 13–22 13,223,
  step 12 166): **1.47 times the top** of its 7,500–9,100, **1.19 times
  the split's ≈ 11,200**, 0.74 of twice the top. Steps 13–22 ran at
  **1.41 times their top** with the other parts' functions, where the
  split took part 1's 1.18. No step over twice its top (step 17 0.98 the
  nearest).
- **Step 25 captured LilyPond's laid-out grobs** (*MS3 · step 25*; its
  sessions' notes in `$LILY_CORPUS/device-runs/2026-10-14-ms3-step25/`).
  `tools/compare/layout.ly` writes after page breaking one line per live
  grob of each system (`grobs.tsv`'s six fields, system, page, x, y,
  x-extent, y-extent, stencil) into `ref/<id>/layout.tsv` of the 42; every
  other file of the capture byte for byte `ref/`'s. **The census: T01 60,
  T03 74, T14 32, T16 43, T19 90 = 299**, 05 §2.1's. `layout.mjs`
  compares two captures (missing, extra and out of order apart; offsets
  and extents to 1e-4; `COMPARED`, empty at step 25, which each step
  grows; `?` fields unreached, counted). The engine writes the same lines
  from a traced compile, natively and from `core.wasm`, every placed
  field `?`. Fixtures `path.ly` (43 live grobs, no live musical column)
  and `y1-unreached.ly` (Y1's 11 unreached types), captured at test
  time. Its gate: **415 Rust and 153 tool tests**, none failed or
  skipped; source 96 lines against step 12's 166; the driver row 104,649
  raw (1.308 of Must).
- **Decisions of part 2:** D108 (the record in the music crate, the grob
  layout's, in one opaque slot), D109 (the capture byte for byte run to
  run), D110 (`scale-layout` in Rust beside `OutputDef`). Step 23 took
  none. Part 3's split took **D111** (layout depends on the skyline crate
  from part 3, amending D104). **The next free D-number is D112.**
- `IMPLEMENT.md` and `PLAN-RUN.md` are committed on `feat/studio-web`
  (c865ff8), in the main checkout, and are not in the worktree. Read them
  there, and don't change anything in the main checkout.

## 1. Run part 3

Run Mother's plan with part 3's twenty step lines from progress.md
(*The plan*, *Part 3: Y1's layout, Y1's other grobs and the layout path
to the core's pages*, the block after *Stop points of part 3*), each in
a fresh session with PLAN-RUN §3's contract. Step 25 is done. **The next
is step 26, *Build and measure skylines in `crates/skyline` and give the
grobs their default skylines***, its line the second in the block. It
needs step 25. Its entry goes at the end of progress.md, after *MS3 ·
step 25*. **The next free D-number is D112.**

| Step | Title | Needs first |
| --- | --- | --- |
| 25 | Capture LilyPond's laid-out grobs of Y1's five as references | step 24 |
| 26 | Build and measure skylines in `crates/skyline` and give the grobs their default skylines (D111) | 25 |
| 27 | Lay out the staff symbol and the staff positions of its referencers | 26 |
| 28 | Lay out axis groups and the vertical alignment | 27 |
| 29 | Solve springs and rods with `Simple_spacer` as LilyPond does | 25 |
| 30 | Lay out paper columns, break alignment and separation items | 26, 29 |
| 31 | Space the columns: the spacing spanner, note spacing and staff spacing | 30 |
| 32 | Space the Paper_score's one line and its loose columns | 31 |
| 33 | Break the System into its line as LilyPond does | 28, 32 |
| 34 | Place one system on its page with `Page_layout_problem` | 33 |
| 35 | Lay out the core's pages: the layout path joins the core | 34 |
| 36 | Profile the translation and make it cheaper (D88) | 35 |
| 37 | Lay out note heads, rests, dots and note columns | 35 |
| 38 | Lay out stems | 37 |
| 39 | Lay out accidentals and ledger lines | 37 |
| 40 | Lay out clefs, keys, the system start bar and side positions: C3 and C4 on Y1's five | 38, 39 |
| 41 | Lay out flags and collisions, with their engravers | 38 |
| 42 | Lay out multi-measure rests, clef modifiers and span bars, with their engravers | 40 |
| 43 | Engrave instrument names and bring T23 into `TRACED` | 40 |
| 44 | Close part 3 of MS3: C3 and C4 on Y1's five, R1, the devices, sizes and handoff | 25–43 |

**Step 26** (its line has the whole scope):

- **Layout depends on `lily_engine_skyline`** (D111,
  `crates/layout/Cargo.toml`), charged to layout's row as D104 charges
  the crate.
- **The skyline calls part 3's files make**, followed inside
  `skyline.cc` and `skyline-pair.cc` (`skyline.txt`: 43 functions, 601
  lines, part 4's ported first): `Building`, `internal_build_skyline`,
  `internal_merge_skyline`, the constructors from boxes and buildings,
  `merge`, `raise`, `shift`, `padded`, `distance` and both
  `internal_distance`, `height`, `max_height`, `right`,
  `set_minimum_height`, `clear`, `Skyline_pair`'s, over
  `crates/skyline`'s `Skyline` and `SkylinePair`; W2's `from_segments`
  and `glyph_skylines` kept where they build the same skyline.
- **The default skylines** (`stencil-integral.cc`, 28 functions, 868
  lines, part 4's): `interpret_stencil_for_skyline` and its builders for
  every stencil expression the five's grobs make, and the 8 `PRIMS` of
  the Grob skyline functions moved to layout (D67; 221 placeholders to
  213).
- **A probe against LilyPond 2.26.0's skylines** (`ly:skyline-*` calls,
  distance and padding among them, and a fixture's grobs'
  `vertical-skylines` and `horizontal-skylines`, points to four
  decimals), natively, from the image natively and in wasm.
- **Unchanged:** W2's glyph skylines (`skylines.ly` point for point), the
  29 traces of `TRACED`, `grobs.tsv`, and step 25's `layout.tsv`
  compare. Without lilypond the tests skip and the step is unverified.
  It stops and asks if a skyline call needs a crate beyond `layout`,
  `skyline` and `svg`, or the core beyond the calls behind the ABI.
- **Its range** (*The steps' lines*): 894–1,089 lines at the proxy,
  1,539 re-estimated, 2,178 at twice the top.

**From step 25 for the steps that follow** (its *Open*): `COMPARED` in
`layout.mjs` is where each step adds its types and fields; the engine
writes every heard grob until suicides and breaking are ported, so a type
compared before then shows its extra grobs; `path.ly` has no live musical
column (steps 29–32); `layout.tsv` has **no System line**, which step 34's
line reads (adding one changes the 42 references of `ref/`: ask).

**How part 3's gate reads** (*How part 3's gate reads*): C3 and C4 are
`score.mjs`' on T01, T03, T14, T16 and T19 through the core's pages,
natively (the compile example's `--out`) and in wasm (`core.mjs`'
`compileInWasm`). C3 is the page count (each has one page and one
system); C4's median ≤ 0.5 and p95 ≤ 2 staff spaces (Goal 0.1, 0.5
recorded). Until the five lay out at step 40, each step compares what it
ports on `layout.tsv`, on the five and on fixtures, natively and in wasm.
B10 and the pixels on the engine's own pages are recorded, not gated. R1
is measured at step 44 on the Android tablet, or on the emulator when the
tablet cannot be reached (answer 2).

**Part 3's lines** (*The steps' lines*, *The re-estimate*): part 3 ports
**20,010 LilyPond lines**, its own 15,591 (15,895 less part 2's 304) and
4,419 of other parts' files first (part 5's one line and one page 2,026,
part 4's skylines, hara-kiri and side positions 2,100, part 2's files'
break functions 293). Steps 26–43 are 12,303–14,971 at the proxies, no
step over 2,500 at twice its top (step 28 2,477, step 33 2,366). At part
2's 1.413, **part 3 ≈ 21,300** (its own share 16,493, the other parts'
4,661), 1.79 of 11,900 and 0.90 of twice it; parts 4 and 5 ≈ 5,700 and ≈
4,500; MS3 ≈ 51,600. Each step's entry compares its source with its
range, as part 2's did. Steps 25, 36 and 44 are apart.

**The placeholders by step** (*Placed here, beyond the parts table*):
`grob.rs`' `PRIMS` 221 → 213 (26), 203 (27), 185 (28), 178 (30), 176
(31), 169 (33), 156 (37), 142 (38), 135 (39), 127 (40), 119 (41), 114
(42): part 3's 91 and 16 of part 4's. `CLASSES` keeps its 117
placeholders; part 3's six are shadowed at 41 (`Collision`,
`Rest_collision`), 42 (`Multi_measure_rest`, `Span_bar`,
`Span_bar_stub`) and 43 (`Instrument_name`), 41 shadowed at the close.

**Placed in part 3** (the same block): the default skylines (26);
`keep-alive-with` and `make-dead-when`, read by part 4's
`request_suicide` ported first (27), `VerticalAxisGroup`'s suicide
compared from 28; `get_pure_minimum_translations` (28); the rods,
`Rod::add_to_cols` (29) and `System::broken_col_range` (33); a group
killed by `Axis_group_engraver`'s warnings kept out of the System (33);
`Output_def::normalize`, a book's parts and toplevel `\bookpart`s, an
output definition's parent (35); the layout path's error reporting in
`stderr` and `diagnostics.json` (35); `translate.html`'s font module and
the layout phase timed apart from the writer (36), the five's first µs
per live grob in desktop wasm (40); `glyph_name`'s ledgers (37); the
`'()` BarNumber's message (40); `stderr` beside C4 (40, 44); T23 into
`TRACED` (43). Where to break, the page breakers and more than one system
or page stay part 5's: a score that would need more systems is laid out
on one line.

**Stop points in part 3** (*Stop points of part 3*, and *The plan*'s). A
step that reaches one stops and asks:

- **The capture changing a reference** (step 25, not reached; step 34
  if it adds `layout.tsv`'s System line): any file of the
  capture other than byte for byte, `capture.json` apart.
- **Module boundaries** (every step; 26, 33 and 35 most of all): layout's
  code in `crates/layout`, the skyline calls in `crates/skyline`, a
  dependency of layout (D111). The music crate naming a layout type or
  holding more of layout than D108's slot, a crate beyond `layout`,
  `skyline` and `svg`, or `crates/core` growing beyond the calls behind
  08 §4.2's ABI (the driver row 1.293 of Must at step 23) stops.
- **A function of another part** (every step): ported first and recorded
  with its lines; one of `PRIMS`' 73 that no part of MS3 ports, or MS4's
  (`beam` and `stem-tremolo` at step 38, which throw by name), reached by
  one of the five or a fixture, stops.
- **05 §5's machinery** is no stop in part 3 (answer 3): a piece built is
  recorded with its lines, and C3, C4 and `layout.tsv` still compare with
  LilyPond. A piece that would cross a module boundary stops.
- **D88's step** (36): a change over a session's 2,500 lines (the part
  that fits made and measured), or any change to a trace, a `\midi`
  playback map or the grob lines.
- **Text widths** (40, 42): a score or fixture failing C4 only where its
  text is placed (W6, part 4's).
- **T23's incipits** (43, answer 5): the embedded scores' layout reaching
  one of `PRIMS`' 73.
- **08 §4.2's ABI and `packages/studio/src/web/engine/`** (25, 35, 36):
  `protocol.ts`, `driver.ts`, `host.ts` or the studio's tests needing a
  change.
- **R1, the tablet, R2 and D88 at the close** (44): R1 over 120 µs
  stops for the user before part 4 is split; the tablet, unlocked, or the emulator in its place for R1 (answer 2); a code row or the
  image over twice its Must reported (R2); a device miss over twice gets
  a step in part 4 (D88).

## 2. By hand

- **The Android tablet:** step 23's runs were made on 2026-10-14, after step
  25 (*MS3 · Android tablet, by hand*;
  `$LILY_CORPUS/device-runs/2026-10-14-fire-max-11/`, its `NOTES.md`,
  `run-tablet.sh` and `sum.mjs`). Translation over twice its Must for T12
  (2.78), T29 (2.74) and T24 (2.52); the first `lily_start` 2.06; first
  collection 1.48, `lsb_restore` 1.89, ns per application 1.76, b5's
  pause 1.32, T29's first parse 1.32; SVG T05 5.1 ms (0.73 of Goal); the
  29 traced 2.92 times step 23's emulator. Step 36 measures on the
  emulator and, when it can be reached, on the tablet with
  `run-tablet.sh`; step 44 runs on the tablet when it can be reached, and
  otherwise the emulator decides R1 (answer 2), with the tablet's ratio
  (2.92) beside it.
- **The real phones** (an A15-class Android phone with Chrome, the iPhone with Safari): none has been run in MS1, MS2 or MS3.

## What MS3 must leave

- MS3's gate (README §5): C3 and C4 on T01, T03, T05, T10, T13, T14, T16,
  T17, T18, T19; B10 on them; W3's pixels against LilyPond's pages (06
  §3.3), with `rsvg-convert -z 2 -b white` and `magick compare`.
- Layout per live grob ≤ 120 µs on the Android device, first at part 3's
  closing step (R1: over 120 stops for the user); the SVG phase against
  README §4.3 (S 20 / 7 ms): within Goal on every page and device at
  part 1's close and on every device run at part 2's (the tablet not
  reached), measured again at each closing step, from part 3 on the
  engine's own pages.
- The tablet's misses, the image and the driver row measured again at
  every closing step; the layout and SVG rows filled (D104).

## Set-up for any session on this branch

```sh
cd .worktrees/engine/packages/engine
npm test                      # cargo +stable, then the tools; needs guile 3.0.11 and lilypond 2.26.0
npm run lint:rust
npm run build                 # Binaryen 133's wasm-opt from the PATH; writes scheme.image with LilyPond 2.26.0
crates/spike/harness/build.sh # the engine, then the studio's site
node crates/spike/harness/serve.mjs   # HTTPS on :8471: scheme.html, translate.html, restore.html, svg.html
```

- `tools/bench/test/wasm.test.mjs`, `image.test.mjs`, `core.test.mjs`,
  `tools/compare/test/c0-cases.test.mjs`, `c1.test.mjs`, `trace.test.mjs`'s
  wasm test and `worker.test.mjs` (which runs the studio's
  `test:engine-corpus`, D101) need a `core.wasm` and images newer than the
  crates' sources. Run `npm run build` first, or they skip.
- `npm test` takes about 5–8 minutes warm (MS3 step 23: 500 and 280 s),
  and a cold run up to 40 minutes (step 22's 2,385 s): run it in the
  background with nothing else of this repository building. Run `npm run
  build` first: a doc edit in a crate's source makes `core.wasm` older
  than the sources and skips the wasm tests. In zsh, `echo =====` fails
  (use `echo ---`). Since MS3 step 1, `test:tools` builds the
  examples once (`tools/examples.mjs`) and the tests run their binaries;
  a test file run alone builds what it needs, and a binary older than its
  crates' sources is built again.
- **LilyPond 2.26.0's sources** are at `~/.cache/lily-engine/lilypond-src`
  (`lily/*.cc`); the installed `scm/` and `ly/` are under
  `/opt/homebrew/share/lilypond/2.26.0/`. MS3's split counted its parts
  with `$LILY_CORPUS/ms3-split/parts.sh` and `count.sh`, and its grob types
  with `census.ly` (`census.txt`); `serif.ly` and `text-width.ly` are the
  text font's probes. Part 2's split counted its grob types, engravers,
  files, placeholders and step lines in `$LILY_CORPUS/ms3-part2-split/`
  (its `README` names each script; `all.sh` makes them again):
  `makers.txt` gives each grob of the 10 with its announcing engraver and
  context, `steps.txt` each step's `lily/*.cc` files, `borrowed.txt` the
  other parts' functions each step ports first. Part 3's split counted
  its files, placeholders, calls, skylines, page path, grob callbacks,
  order and re-estimate in `$LILY_CORPUS/ms3-part3-split/` (`README.md`,
  `all.sh`, 52 checks): `order.txt` gives each of steps 25–44's files and
  the other parts' functions it ports first, `prims-part3.txt` the 91
  `PRIMS` by file, `skyline.txt` the 43 skyline functions, `page.txt`
  the one line and one page, `grobdefs.txt` `define-grobs.scm`'s
  callbacks of Y1's grobs, `reestimate.txt` the steps' lines.
- **W3's pixels** (part 1's step 7 on): Homebrew's `rsvg-convert` 2.62.0
  (`-z 2 -b white`) and ImageMagick 7.1.2-0's `magick compare -metric PAE`
  and `-metric AE`, in `/opt/homebrew/bin`. The tests skip without them,
  and a run that skipped them has not verified the writer.
- In `packages/studio`: `node esbuild.mjs --tests && node --test
  out/test/engine.test.js` (19 tests) and `out/test/engine-corpus.test.js`
  (its corpus part skips without `LILY_ENGINE_CORPUS`, by design).
- `cargo +stable run -p lily-engine-core --example compile -- [--time]
  [--trace] --out <dir> <file.ly>…` compiles files through the core
  natively, one engine for all, answering its asks from disk.
- Hosts restart a compile after each answer with the **same revision and
  root** (D89), or lose the saving.
- **Check that no stale server holds a port** before serving:
  `lsof -nP -iTCP:8471 -sTCP:LISTEN` (step 42 found step 40's `serve.mjs`
  still on :8471). Stop the servers when the runs are done.
- **The tablet** (the Android tablet):
  1. `adb shell svc power stayon true` and `adb shell input keyevent
     KEYCODE_WAKEUP`; `usb` is not enough, since it charges over AC.
  2. Unlock it, with Silk in front. Its serial
     is `<serial>`; use `adb -s` when the emulator is up too.
  3. `adb shell cat /proc/$(adb shell pidof com.amazon.cloud9 | cut -d' '
     -f1)/cpuset` gives `/top-app`.
  4. `serve.mjs --http --port 8481`, the probe with `--http --port 8482`,
     `adb reverse tcp:8481 tcp:8481` and `tcp:8482`, and `adb forward
     tcp:9333 localabstract:amazon_silk_devtools_remote`; `drive.mjs cdp
     http://localhost:8481 http://localhost:8482 <label> 3`, then
     `drive-restore.mjs cdp http://localhost:8481 <label> 3`, then
     `drive-svg.mjs cdp http://localhost:8481 <label> 3`.
  5. `stayon` back to `false` afterwards, and the forwards removed.
- **The Android emulator** (step 42, not a stand-in for the tablet's
  budgets): `ANDROID_AVD_HOME=<the AVD folder>`, `emulator -avd
  mother_api36 -no-window -no-audio -no-snapshot` (a Pixel 7, Android 16;
  there is no `Pixel_7_API_36`), its system Chrome 133; `adb -s
  emulator-5554` throughout, since the tablet may be attached too. Skip
  Chrome's first run with `echo "_ --disable-fre --no-first-run" >
  /data/local/tmp/chrome-command-line` and `am set-debug-app --persistent
  com.android.chrome`; DevTools by `adb forward tcp:9333
  localabstract:chrome_devtools_remote`. Afterwards remove the forwards and
  the file, `am clear-debug-app`, and `adb emu kill`.
- **The Simulator:** the **iOS 26.3** iPhone 17 Pro
  (`<udid>`), which trusts mkcert's root CA.
  `sim.mjs` opens the three pages, `sim-restore.mjs` `restore.html` and
  `sim-svg.mjs` `svg.html`
  through `xcrun simctl openurl` (`SIM=<udid>` when another simulator is
  booted); the results arrive in `serve.mjs`'s `--results` file. Shut it
  down afterwards.
- **Desktop:** `drive.mjs chromium|webkit https://localhost:8471
  https://localhost:8472 <label> 3`, and `drive-restore.mjs` (with `…
  "thread=page&workers=1&modes=start" --fresh --throttle 4` for throttled
  Chromium), and `drive-svg.mjs chromium|webkit https://localhost:8471
  <label> 3` (with `… "thread=page" --throttle 4` throttled), from
  `~/.nvm/versions/node/v24.14.0/lib`, where Playwright 1.59.1 resolves.
  Check `uptime` first. Throttled Chromium is not the tablet: it
  underestimates the tablet's first restore (1.53 times) and overestimates
  its SVG writing (1.22–1.26 times); measure on the tablet.
- Keep device results in `$LILY_CORPUS/device-runs/` (default
  `~/.cache/lily-engine`), not in the session's scratchpad. MS2's closing
  results and scratch scripts are in `2026-10-07-ms2-step42/` and
  `2026-10-07-ms2-step42.jsonl` (its `NOTES.txt` says which is which):
  `run-desktop.sh`, `drive.mjs`, `drive-restore.mjs`, `sim.mjs`,
  `sim-restore.mjs`, `probe-serve.mjs` (`gen.mjs <engine> <dir>` rewrites
  its references from the build first), `sum.mjs`, `sum-restore.mjs`,
  `sum42.mjs` and `exports.mjs`. Part 3's are in `2026-10-06-ms2-step28*`,
  step 31's restore probe in `2026-10-07-ms2-step31/`. MS3 part 1's close is
  in `2026-10-08-ms3-step10/` and `2026-10-08-ms3-step10.jsonl` (its
  `NOTES.txt`): `gate.sh`, `rest.sh`, `run-desktop.sh`, `run-tablet.sh`,
  `run-sim.sh`, `sim-svg.mjs`, `sum-svg.mjs`, `lines.sh` and
  `lines-sum.sh` beside step 42's scripts. MS3 part 2's close is in
  `2026-10-13-ms3-step23/` and `2026-10-13-ms3-step23.jsonl` (its
  `NOTES.md`, one section a session): `session1-gate.sh` and
  `session1-tally.sh` (the build, a run, its count), `session2-rest.sh`,
  `session3-counts.mjs`, `session3-t15.mjs`, `session4-sizes.mjs` and
  `session4-names-cmp.mjs` (cores compared by function name), step 10's
  device scripts as `session5-*` with `session5-sum.mjs`,
  `session6-run-emulator.sh` (the tablet's runs too, with its serial and
  Silk's socket) and `session6-sum.mjs`, and `session7-lines.sh`. Part
  3's split is in `2026-10-14-ms3-step24/` (its `NOTES.md`, one section a
  session), with `session1-run-tablet.sh`, the tablet's runs ready and
  not run there. They ran from `2026-10-14-fire-max-11/` (its `NOTES.md`):
  `run-tablet.sh`, `rerun-restore.sh`, `sum.mjs` (the tablet against step
  10's tablet, step 23's emulator and its projections) and `runs.jsonl`.
- Background `serve.mjs` runs stop at the tool's time limit. Give them a
  long timeout.
- Guile 3.0.11 is at `/opt/homebrew/bin/guile`, and Binaryen 133 is from
  Homebrew.

## Carried, not urgent

- From MS3 part 2 (progress.md, *MS3 · step 23*, *Open*, and steps
  12–22's): the core's download 8,986 bytes over its Goal; the unnamed
  functions' +827 bytes since step 10 not retraced, and the music model's
  +10 (step 20) and a function fewer (step 19) not looked into;
  `sizes.mjs`' N5 row not naming `marks`; `UNMADE`'s two engravers (part
  4); Emmentaler's text in 4 of the 40 text prints (part 4); the volta and
  ottava prints (part 4); `Staff_collecting_engraver` (`stavesFound`) in
  no part; BarNumber's prints not compared; the grobs' parents not
  compared (not in `grobs.tsv`); the engine's `\fine` broadcasting no
  `fine-event`; `\breve` and `\longa` read as whole notes;
  `Timing_translator`'s doubled `\polymetric` warnings; `Tie_engraver`'s
  delegation until drum and tab heads are made; the paths with no command
  column and the acknowledged non-items and non-spanners; the X
  callback's and `stencil-height`'s type-error names; `ref/grob-calls/`
  not recaptured; the stable sorts of `neighbors` and `busyGrobs`;
  `Beam_engraver` and T15's Flags (MS4); `keep-alive-with` and
  `make-dead-when` are part 3's now (step 27, *1* above).
- From MS3 part 1 (progress.md, *MS3 · step 10*, *Open*): T16's page 0.1
  KB over its compact 4 KB; the data section (249,739 bytes at step 23,
  `dec2flt`'s table among it) charged to no row; the driver row's float
  formatting and parsing and the bench exports (R2's leads); `slur` and
  `bezier_sandwich` (MS4); W5's packs on demand and the brace parts'
  brotli (part 4); W4 (MS6); W6 beyond Emmentaler's shaping (MS4);
  `Offset::length`'s `libm` callers; the Simulator's slow fresh instances
  on `scheme.html`.
- From MS2 (progress.md, *MS2 · step 42*, *Open*, which lists them
  whole): 08 §4.1–§4.2 still say the page sends texts (D102); the driver
  and MIDI's playback map (MS4, D103); a `\book`'s messages' place and
  `warning-as-error` (D99); `ly:set-option`'s options persisting across
  compiles; `timing.ly`'s earlier performances (D99); cases 17, 18 and
  30's doubled warnings; `completionBusy`, `Drum_note_performer` and a tie
  translator with no maker (D98); the lyric translators (MS3, MS4) and
  `ignoreMelismata`; a dying voice's lyrics iterator (D97); an expected
  text past its message (D96); an assignment's `#` form defined after the
  next token; a tagged `\change`; the partcombine part's GOOPS
  (`\autoChange`, `\partCombine` and its two, `\addQuote`); `syntax.ly`'s
  `\book` scores node by node; a template cut short beginning with a
  direction (D93); `\etc` after a wrong property path; an `\include` read
  again in the incl state; `(lily)` exporting the syntax constructors
  (D93); a clone with no location and an `\include` in a template (D92);
  `ly:parser-clear-error`; `write` of music; `\lyricsto Type = id` (P12,
  MS8); step 31's throttled gap, the memory grown once, no fixed point of
  a native restore and write; an include's candidates at one stop; the
  studio's page without `<datadir>` files (K4).
- From MS2's parts 2 and 3: a file's `define` of an init variable shadows it
  (D78); init origins in diagnostics; variable objects; an output
  definition's parent; `qr`'s `(rnrs bytevectors)` and `(ice-9 iconv)`;
  `Cx::global` by name; first-class variables; hash table order; three
  copies of `load_library`; PCRE beyond the subset; a chord-mode pitch
  argument, `DRUM_PITCH` as an argument, mode heads as a `\set`'s value;
  deprecated properties; properties interned per translation; silent
  `Grob_property_info` errors; the trace without overrides; no Scheme face
  that drives the loop (D70); `ly:event-set-property!` shadowing; the
  iterator arena's slots never reused; `ly:broadcast` outside a
  translation (D85); a translator name found in the classes first (D86);
  grobs of C++ classes (MS3); a stream event's printed form.
- From MS1: the curried `define`'s `(define (f . 1) 2)` wording, Guile's
  `version` in `(guile)`; `ly:dir?`; literal strings still mutable; errors
  inside `#{ … #}` from LilyScan; a correctly rounded libm and the missing
  number functions; R11's write barrier and undo log; the printer's cyclic
  labels; uninterned symbols.
- From MS0: B7 on the tablet; M8's background glitches; ▶ and ■ as "?" in
  the iOS Simulator; H6 and M8 on the real phones; the worker's heap over
  4 MB; W5's braces across parts, and W6's shaping rules; 06 §5.3's AGPL
  question; chapter 06 writes `c5.tsv`; C4's links whose first child is
  not a `translate`; libstdc++'s heap for SMF; D58's port to
  `ScorePreview`; `IMPLEMENT.md`'s old spike path.
