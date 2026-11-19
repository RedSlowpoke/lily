# The runway's plans

Each section is one plan: what it builds, what it needs, the open items it
inherits (ids of [`OPEN.md`](OPEN.md)), its gate, budgets, stop points,
estimate, and what its closing step leaves. "P:n" is progress.md's line n
at a3b67f4; "README §n" is `docs/research/wasm-engine/README.md`. A
plan's split step refines its own section here (PROTOCOL §1).

Every plan also carries the runway's stop points (PROTOCOL §6) and the
closing step's checklist (PROTOCOL §4).

---

## R1 — MS3 part 3, steps 36–44: Y1's five laid out, and R1

**State:** **closed by MS3 step 44 (2026-10-22); stopped for the user at
R1** (205.7 µs a live grob on the emulator, 576–638 on the tablet by
ratio; LEDGER.md's Next action holds the question). The gate passed
natively and in wasm with 29 traces and T23's pin (answer 5; the step
line's 30 was written before step 43) and `grobs.tsv`'s 1,913 lines (1,903
grobs and 10 Systems); progress.md's MS3 step 44 entry. Step lines:
P:17327–17335 (title | scope | files | minutes | why); the part's notes:
P:16755–17310 (*Part 3*), its stop points P:17247–17310.

| Step | Title | Re-estimate (lines) | Needs |
| --- | --- | --- | --- |
| 36 | D88: the translation profiled and made cheaper (no LilyPond file) | none: outside the proxy, stops past 2,500 | 35 |
| 37 | Note heads, rests, dots and note columns (13 PRIMS → layout, 169 → 156) | 1,511 | 35 |
| 38 | Stems (14 PRIMS, → 142; MS4's beam and tremolo calls throw by name) | 1,479 | 37 |
| 39 | Accidentals and ledger lines (7 PRIMS, → 135; the five's current stop) | 1,178 | 37 |
| 40 | Clefs, keys, the system start bar, side positions: **C3 and C4 on Y1's five** | 853 | 38, 39 |
| 41 | Flags and collisions, with their engravers | 1,501 | 38 |
| 42 | Multi-measure rests, clef modifiers, span bars, with their engravers | 1,345 | 40 |
| 43 | Instrument names; T23 into `TRACED` | 201 | 40 |
| 44 | **Close part 3**: C3/C4 on the five, R1, the devices, sizes, handoff | none | 25–43 |

- **Inherits:** A1, A2, A3, A6, A7, A8, A9, A11, A16, A20, A21, A22, B5, C1,
  C5, D3, H1–H5, H7, H8, H10.
- **Gate (step 44, P:17335):** C3 and C4 on T01, T03, T14, T16, T19
  through the core's pages, natively and in wasm (C4 median ≤ 0.5, p95 ≤ 2
  staff spaces; Goal 0.1/0.5 recorded); `layout.tsv` LilyPond's on the
  five; pixels (AE, PAE) and B10 recorded; Y1's 11 unreached grobs on
  fixtures recorded; the 30 traces of `TRACED` (T23 in), `grobs.tsv`'s
  1,913 lines, part 1's gate, MS2's gate; `CLASSES` 117 placeholders with
  41 shadowed, `PRIMS` 114; each laid-out score's stderr beside its C4.
- **Budgets:** layout per live grob (**R1**: 120 / 40 µs, on 299 live
  grobs: T01 60, T03 74, T14 32, T16 43, T19 90), the SVG phase (S 20 / 7
  ms), translation of T12, T29, T24 (S 60, L 250 ms Must) against step
  23's and step 36's, the tablet's misses under twice, sizes against step
  23's (core.image 456,765 brotli; driver row split; layout row; data
  section).
- **Stop points (P:17247–17310):** R1 over 120 µs (the five together or
  the fit's cost per grob) stops for the user before R2's split; step 36
  past 2,500 lines or changing an output; module boundaries; a function of
  another part; T23's incipits reaching one of `PRIMS`' 73 (answer 5);
  text widths at steps 40 and 42; the ABI and the studio's engine files.
- **Devices:** the Android tablet for R1, or the emulator `mother_api36`
  deciding it with the tablet's ratio (2.8–3.1) beside it (answer 2);
  desktop Chromium, WebKit and the iOS 26.3 Simulator.
- **Exit:** LEDGER: R1's result (met, or the user's question with the
  figures and 05 §5's unbuilt machinery named); D88's list for R2's split;
  OPEN updated; `NEXT-SESSION.md` not rewritten (D200).

## R2 — MS3 part 4: Y2, W5, W6's Emmentaler shaping, the first restore

**State:** **closed by MS3 step 57 (2026-10-23); stopped for the user at
R1 and answered the same day.** The gate passed natively and in wasm with
nothing skipped: C3 and C4 on Y2's eight at Δ 0, `layout.tsv`
LilyPond's on their 608 live grobs, W5's check, the fixtures, and every
earlier gate in both full runs (223 tool tests, 74 Rust result sets with
737 passed). Layout's classes shadow 54 placeholders and `PRIMS` holds 85
(29 gone): the user's answers added four classes and one `PRIMS` to the
step lines' 50 and 28. **R1 on the eight: the iPhone 134.9 µs
(1.12 of Must), the emulator 224.7 (1.87)**, the Android tablet 692.6
beside them (D206). Answered: **R3's split's first step is Y2's skyline
cost** (OPEN A42: libm's software fma and the padded skylines of
outside-staff placement); the text packs' size (E14) is recorded for R8.
Lines: 8,894 (0.989 of the split's 8,989). See progress.md's MS3 step 57
entry. Split by MS3 step 45 (2026-10-22): the counts
(`$LILY_CORPUS/ms3-part4-split/`), the order, the budgets, the stop points
and the step lines **MS3 · steps 46–57** are in progress.md's *The plan*,
*Part 4*.

**Settled with the user (2026-10-22, MS3 step 45;** in full in
progress.md's *The plan*, *Settled with the user at part 4's split*):

1. **R1's stop: option 2.** Part 4's first step is 05 §5.1's machinery:
   grob.cc's and staff-symbol-referencer.cc's preludes as Rust over
   `Host`, then the callback table; typed slots behind D203's table. It
   is measured on the five before Y2, with C3, C4 and `layout.tsv`
   still LilyPond's there; option 3's question (the budget) comes back
   when it is measured.
2. **B9:** the core writes `LilyPond Serif` (D106's family) for the
   page's text.
3. **E7:** the text packs carry outlines, so text skylines are
   LilyPond's (A27).
4. **Devices:** the Android tablet and the iPhone can be
   attached at R2's measurements; the A15-class Android is not
   available. Their 2026-10-22 figures (LEDGER.md) replace the tablet's
   projections.
5. **E8:** the `libm` crate's `sin` is kept: skylines of circles and
   ellipses LilyPond's to 1e-9, not bit for bit; a gate that fails only
   by `sin`'s last bit stops for the user.

- **Builds (P:15267, P:15290):** Y2: skylines of every stencil with W2's
  procedure, outside-staff placement, side positions, staff distances,
  empty staves (`hara-kiri-group-spanner`), `interval-minefield`, L5's
  script, fingering, arpeggio, hairpin, line-spanner, ottava-bracket,
  volta-bracket, horizontal-bracket and bracket, and the engravers of Y2's
  grobs the 10 do not reach. `CLASSES`' 9 placeholders (`Script`,
  `Script_column`, `Fingering`, `New_fingering`, `Fingering_column`,
  `Arpeggio`, `Span_arpeggio`, `Metronome_mark`, `Horizontal_bracket`);
  `PRIMS`' 44 in 12 files less the 16 part 3 takes.
- **Outside the proxy, estimated by its split:**
  - **W5**: packs on demand: the host's fetch of the packs T06, T09, T22
    and T29 ask for, a missing pack's diagnostic, `scripts/build.mjs`'
    manifest naming them (D105); T10's Emmentaler-13 and T13's
    `brace200` from a pack the core does not hold.
  - **W6's Emmentaler shaping** (answer 1 at part 2's split): GPOS
    `kern`, GSUB `cv47`, `ss01`, `tnum`, `dlig` in W1's Emmentaler packs;
    the pack format changes, so it takes a decision (D203 or later); it
    comes before C4 on T10, T13 and T18 and fixes the 8 `EMMENTALER_TEXT`
    markups.
  - **A cheaper first restore** (D201): `lily_start` on the tablet,
    102.9 ms (2.06 of 50); the lead is what a fresh worker pays over a
    warm restore (26.6–37.8 ms), not a smaller image.
- **Inherits:** A4, A5, A12, A17, A18, A23, B2's `get_neighboring_staff`,
  D2, E1, E3, E4, E6, J1; and B9's and E7's answers (above) to build.
- **Questions its split asked the user** (answered above): whether the
  text packs carry outlines (E7); whether skylines of circles and
  ellipses need Apple's `sin` bit for bit (E8); which text family the
  core writes (B9), since R3's MS3 gate reads W3's pixels on the
  engine's own pages.
- **Steps** (MS3 · 46–57; progress.md's *Part 4*, each with PLAN-RUN
  §5's five fields): **46 the machinery** (grob.cc's and
  staff-symbol-referencer.cc's preludes as Rust over `Host`, the
  callback table, typed slots behind D203's table; R1 on the five),
  **then option 3's stop**; 47 Y2's references and fixtures; 48 the
  outside-staff ground (skylines' rest, side positions, the interval
  minefield, empty staves); 49 hairpins and brackets; 50 W5; 51 W6's
  Emmentaler shaping with E7's outlines and B9's family, **C3 and C4 on
  Y2's eight**; 52 line spanners; 53 scripts and fingerings (may take two
  sessions: the scripts, then the fingerings); 54 arpeggios and the
  metronome mark; **55 D88's translation step** (OPEN C1: T12, T29, T24
  over twice on the tablet); 56 the first restore (D201); 57 R2's close.
- **Gate:** **C3 and C4 on Y2's eight** (the five + T10, T13, T18);
  W5's check (06 §9); everything R1 left passing.
- **Budgets** (the 2026-10-22 device figures, not projections; since D206
  the iPhone and the emulator decide R1, the Android tablet
  reported beside them): R1 810.4 µs a live grob on the
  tablet (6.75 of Must), 123.7 on the iPhone, measured again at 46 on
  the five and at 57 on the eight's 608 (+ T10 121, T13 45, T18 143); by
  step 44's floor the tablet stays near 190–220 µs with all of 05 §5.1
  built. Translation on the tablet T12 154.0 ms (2.57), T29 682.2 (2.73),
  T24 553.6 (2.21): step 55 brings each under twice. The first restore
  72.7 ms (1.45): step 56. The tablet's other misses under twice (D1),
  `lsb_restore` the nearest (1.96). Sizes: B2's module rows for the
  packs (each Emmentaler size 60 / 40 KB, braces 25 / 23, text packs
  20 / 12); `core.image` 1.137 and the driver row 1.526 / 1.662 of Must.
- **Stop points:** **option 3's question after step 46** if the tablet is
  over 120 µs (answered 2026-10-23: 3, then 1; D206); R1 over 120 µs
  on the iPhone or the emulator at the close (D206); step 46 past 2,500 lines or
  changing an output; text widths beyond Emmentaler's shaping and E7's
  outlines; the pack format changing without its decision (step 51);
  a gate failing only by `sin`'s last bit (E8); module boundaries (the
  slots and the table are layout's; `host.ts` and the studio unchanged; D108's
  downcast at 55); D88's step or the first restore past 2,500 lines; a
  device that cannot be reached when R1 or D88 is decided. E8 is
  answered, so it is no stop point of its own.
- **Estimate:** **≈ 9,000 lines at its split** (8,989: the ported lines'
  4,469 at the top × part 3's rate 1.652 = 7,384, the models 1,605);
  ≈ 5,700 at part 3's split without the machinery, W5, W6, D88's step
  and the restore. No step over 2,500 at twice its top (the largest, 53,
  2,354); step 53 at step 31's rate 2,929, so two sessions. MS3 ≈ 59,700
  (1.635 of 36,500).

## R3 — MS3 part 5: Y2's skyline cost, Y3, and MS3's close

**State:** **split by MS3 step 58 (2026-10-23); step 59 done
(2026-10-24, D216: R1 on the eight the iPhone 131.6 µs, the emulator
214.9); step 60 done (2026-10-24, D217: several lines at breaks a test
gives, broken-spanners.ly LilyPond's, H9's System lines, `ref/`
recaptured); step 61 done (2026-10-24, D218: the line breaker and
page-breaking.cc's line side, T05's two systems LilyPond's); step 62
done (2026-10-24, D219: `ly:optimal-breaking` in place of `one_page`,
T17's and T02's pages LilyPond's, a prob's `Line_details` brought
forward); step 63 done (2026-10-24: titles and top-level markups on
the pages, the titles fixture LilyPond's pixel for pixel); step 64
done (2026-10-24, D220, D221: page markers and labels, B7's roman page
numbers, B8's errors reported, Guile's promises; the page-markers fixture
LilyPond's pixel for pixel); step 65 done (2026-10-24: MS3's gate on
the 10 passed natively and in wasm with no fix, as tests; E13's cause
found); step 66 done (2026-10-24), R3 and MS3 closed: the gate in both full runs (756 Rust tests, 234 tool tests, none skipped), MS3's summary in progress.md; R1 on the 10's 1,461 on the emulator 250.1 µs (2.08), the devices not attached (the user's answer: the emulator in their place); the user's answers: R1's lead is R5's first step, R4's split next; Lane C not opened** (LEDGER.md). The counts (`$LILY_CORPUS/ms3-part5-split/`), the order, the
budgets, the stop points and the step lines **MS3 · steps 59–66** are in
progress.md's *The plan*, *Part 5*.

**Settled with the user (2026-10-23;** in full in progress.md's *The
plan*, *Settled with the user at part 5's split*; the first at R2's close,
the other four asked at the split, each the recommended option):

1. **Part 5's first step is Y2's skyline cost** (OPEN A42), measured on
   the eight before Y3: an exact fma cheaper than libm's in wasm, or fused
   only where LilyPond's clang fuses; padded skylines kept across
   outside-staff moves; then 05 §5.1's remaining pieces (D3's list) as the
   step's lines allow. R1 on the eight there (the iPhone 134.9
   µs and the emulator 224.7 at R2's close) and on the 10 at MS3's close.
   The text packs' size stays with R8 (E14).
2. **Devices:** the Android tablet and the iPhone are attached at
   step 59 and at MS3's close; D206 holds (the iPhone and the emulator
   decide, the tablet beside them).
3. **A37 and A38 belong to R9 (Y8):** none of the 30 reaches
   `ly:bezier-extent` or breathing-sign.cc's divisiones and `finalis`.
4. **C5 is recorded; R8 owns it:** under twice on D206's two devices
   (T29's first compile 1.10 and 1.90 of 150 ms), so D88 gives it no step.
5. **H9's recapture is approved** for step 60 (the System's line in
   `layout.tsv` changes the 42 references); any other byte of `ref/` that
   changes still stops.

- **Builds (P:15268, P:15291):** A42's skyline cost; Y3:
  `constrained-breaking` (`solve`, `best_solution`, `fill_line_details`
  with the System's `pure_refpoint_extent` and footnote and in-note
  heights), page-breaking.cc's line and page sides less the other
  breakers, `ly:optimal-breaking` (`PRIMS`' one part 5 entry) with
  optimal-page-breaking.cc, page-spacing.cc and page-spacing-result.cc in
  place of step 35's `one_page` call; broken items and spanner slices
  over several systems (the items' pure cache; break substitution's
  paths, all of whose units step 33 ported); titles and top-level markups
  as page elements; page markers and labels (`Page_marker`,
  `set_labels`); O3's `paper-book` remainder (`output`, `output_aux`).
  **Counted:** part 5's files 6,256 lines, the engine has 2,252 (part 3's
  2,026, step 35's 208 beyond its line, 18), 4,004 left: 3,456 ported,
  359 not (the other breakers, R9's; performances, MS5's; the other
  backends), 189 between units; 91 lines of other parts' files first
  (system.cc 73, item.cc 18).
- **Inherits:** A7's check, A10's check, A14, A24, A25, A39 (where
  touched), A40, A42, B1, B2, B3, B4's rest, B6, B7, B8, B11, B12, B13,
  C1 (recorded), C5 (recorded, R8's), D1, D2, D3, D4, D7, E4, E10, E13,
  H4, H9, J2; each placed by step in progress.md's *Part 5*. A37 and A38
  go to R9 (answer 3); J1 is done by the split.
- **Steps** (MS3 · 59–66; progress.md's *Part 5*, each with PLAN-RUN §5's
  five fields): **59 Y2's skyline cost** (A42; R1 on the eight); 60 broken
  items and spanner slices with their pure caches, H9's System line; 61
  constrained-breaking's line breaker with `fill_line_details` whole and
  page-breaking.cc's line side (**T05's two systems**); 62
  `ly:optimal-breaking` in place of `one_page` (**T17's two pages**); 63
  titles and top-level markups as page elements (on a fixture: none of
  the 10 has titles); 64 page markers, labels, O3's remainder, B7, B8 (a
  decision); 65 **MS3's gate on the 10**; 66 MS3's close.
- **Gate — MS3's whole (README §5):** **C3 and C4 on the 10** (T01, T03,
  T05, T10, T13, T14, T16, T17, T18, T19); **B10** on them; **W3's pixels**
  against LilyPond's pages (06 §3.3); natively and in wasm.
- **Budgets** (step 57's figures): R1 on the eight the iPhone 134.9 µs
  (1.12), the emulator 224.7 (1.87), the tablet 692.6 beside them;
  measured at 59 on the eight and at 66 on the 1,461 live grobs (+ T05
  502, T17 351). D88's translations within Must on D206's two (the tablet
  T12 2.84, T29 2.63 beside them); T29's first compile (C5) 1.10 / 1.90;
  the first restore 1.66 on the tablet; the SVG phase; every row and the
  image against README §4.1 (the image 1.138, the driver row 1.687 /
  1.803). A device miss over twice goes to MS4's split (P:15655).
- **Stop points:** R1 over 120 µs on the iPhone or the emulator at the
  close (66); step 59 changing an output (a last bit among them) or
  passing 2,500 lines; `ref/` changing beyond the System's line and the
  broken pieces (60); an output changing where a step says none does; a
  function of no part (A37, A38 R9's); B8's reporting without its
  decision (64); text widths (63, 65); E8's `sin`; module boundaries; a
  device that cannot be reached at 59 or 66; D88 at the close.
- **Estimate:** ≈ 4,500 (P:17110), less part 3's 2,026, ≈ 4,767 at part
  4's rate 1.50 (step 57), with the machinery step beside it; **≈ 5,900
  at its split** (5,895: the ported lines' 3,547 at the top × part 4's
  rate 1.50 = 3,978, the models 1,917, step 59's 1,151 among them). No
  step over 2,500 at twice its top (the largest, 62, 2,298). MS3 ≈ 60,200
  (1.650 of 36,500).
- **Exit:** **MS3's summary** above its step entries (IMPLEMENT's *Done
  for a run*, as MS2's at P:6561): what was built with commits, each gate,
  each budget against README §4, deviations, decisions left to the user,
  the device runs by hand; LEDGER and OPEN; the lines of MS3 against
  30,000–36,500 and the ≈ 51,600, ≈ 59,700 and ≈ 60,200 re-estimates;
  Lane C asked once (GRAND-PLAN.md).

## R4 — MS5: MIDI

**State:** **closed by MS5 step 9 (2026-10-25)**: R4's gate passed in
both full runs (C2 = 1.0 with velocities on the 30 and M01–M12, C2b on
the 42, C1 against `timing.ly`'s map, natively and in wasm); the Android
tablet and the iPhone attached: R1 on the 10 the iPhone 146.5
µs (1.22), the emulator 259.5 (2.16), recorded (answered at MS3's
close); T29's performance and file within Goal on every device; no D88
miss over twice on D206's two; the MIDI row 17,133 / 8,093; no size over
twice Must; **3,711 lines, 0.83 of the split's 4,448**; MS5's summary in
progress.md; next **R5's split**. Split by MS5 step 0 (2026-10-24); step 1 done
(2026-10-24, 1,410 lines: the framework and the core's MIDI, C2 = 1.0
on its fixture and the 14); step 2 done (2026-10-24, 641 lines:
`Dynamic_performer` and the equalizer, every velocity of the 42
LilyPond's); step 3 done (2026-10-24, 623 lines: keys, time signatures,
programs, controllers, pedals; whole bytes on the 26 of steps 1–3 and
M06, M10, T19); step 4 done (2026-10-24, 351 lines: lyrics, markers,
ties and breaths; whole bytes on the 10 of step 4 and T29, 40 of the 42);
step 5 done (2026-10-24, 114 lines: `Drum_note_performer`,
`Beat_performer`'s Scheme in the image, the channels checked; 41 of the
42 whole; **the user's answer: the part combiner a step of R4's own,
step 6a**, for M12, after step 6); step 6 done (2026-10-24, 176
lines: `Percent_repeat_iterator` and `Fine_iterator`, `\unfoldRepeats`
over every repeat type and `\articulate` LilyPond's; 41 of the 42
whole); step 6a done (2026-10-24, 293 lines: GOOPS's generic
functions, `ly:make-global-context` and `ly:interpret-music-expression`,
`Part_combine_iterator`; M12 LilyPond's, **all 42 whole**); step 7
done (2026-10-24, 19 lines: the MIDI row 21,250 / 9,070 with the
performers and T29's performance and file 43.8 ms on the emulator, both
under Goal, no cut; **G1 as D222**); step 8 done (2026-10-24, 70
lines: **R4's gate passed** natively and in wasm, kept as tests; M01's
expected warnings and `Midi_walker`'s two cases fixed); step 9 done
(2026-10-25, 0 lines: R4's close, the devices measured). In the main lane (lane A, the user's answer), after R3 and
before R5 (PROTOCOL §5). The counts (`$LILY_CORPUS/ms5-split/`), the order,
the budgets, the stop points and the step lines **MS5 · steps 1–9 and 6a** are in
progress.md's *MS5*, *The plan*.

**Settled with the user (2026-10-24;** in full in progress.md's *MS5*,
*Settled with the user at R4's split*; each the recommended option):

1. **Lane A: the main lane**, `.worktrees/engine` on `feat/engine`;
   `.worktrees/engine-midi` not opened, no decision block reserved.
2. **G1 (D103): the driver ties the playback map to a MIDI file at R8**, as
   `read_playback` does. R4 makes the core write `<name>.midi` beside the
   map; step 7 records the decision refining D103; `protocol.ts`,
   `driver.ts`, `host.ts` and the studio's tests stay a stop point.
3. **The devices at R4's close only** (step 9): the Android tablet and the
   iPhone, attached on asking; step 7 measures on desktop and
   the emulator.

- **Builds (README §5, MS5):** M2–M6: the performer framework, dynamics
  with the equalizer, controllers and tempo, marks, drums and channel
  mappings, `\unfoldRepeats` and `articulate.ly` as a library part;
  `crates/smf` linked into the core (it is "not linked" today).
  **Counted:** MS5's 27 `lily/*.cc` files 4,184 lines; the engine has 238
  (the framework's performer groups 109, D98's beam and slur performers 55,
  `crates/smf`'s 74); **3,946 left: 3,611 ported** by steps 1–5, 118 not
  (destructors and marking), 216 the engine's; 16 callees outside MS5, all
  the engine's; no function of no part; M6 and the equalizer are Scheme in
  the image or served on `\include`. M1's writer (525) is MS0's.
- **Inherits:** G1 (answered: R8; step 7's decision), G2 (`Lyric_performer`
  step 4, `Drum_note_performer` step 5, the channel wrap ported at 1 and
  checked at 5), C2's `Drum_note_performer` (5), D3 (R1 on the 10 on the
  devices, step 9), D1, D2, D7, H4, E4, E10 (recorded at 9).
- **Steps** (MS5 · 1–9; progress.md's *MS5*, each with PLAN-RUN §5's five
  fields): **1 the performer framework and the core's MIDI** (M2,
  `crates/smf` linked into the core, the first tempo and every staff's
  channel; a plain-notes fixture; C2 on 14); 2 `Dynamic_performer` and the
  equalizer (+ M08, T18, T25); 3 keys, time signatures, controllers, pedals,
  programs (+ 9; **whole bytes from here**); 4 lyrics, ties, marks (+ 10);
  5 drums and channels (+ M06, M12, T07, T29); 6 `\unfoldRepeats` and
  `articulate.ly` (+ M10, T19: all 42); 7 the budgets and G1's decision;
  8 **R4's gate**; 9 R4's close.
- **Gate:** C2 = 1.0, velocities included, on the 30 and M01–M12; C2b on
  all 42, ties at one tick normalised; C1 against `timing.ly`'s map;
  natively and in wasm (step 8; both full runs at 9).
- **Budgets:** MIDI ≤ 120 KB raw / 45 KB brotli in the core (Goal 80 /
  30; the row is 0 at the split, `crates/smf` not linked, and step 7 names
  the performers' functions into it); T29's performance after an edit ≤
  300 / 100 ms (never measured; step 7 on desktop and the emulator, step 9
  on the devices); D88's translations (every compile translates a `\midi`)
  at 7 and 9; R1 on the 10 on the devices at 9 (the emulator 250.1 µs at
  MS3's close), recorded.
- **Stop points:** `protocol.ts`, `driver.ts`, `host.ts` or the studio's
  tests changing (G1 is R8's; the studio's MIDI path, D54, is the server's
  until R8); the music crate naming a layout type, or a crate beyond
  `crates/smf` linked; a reference of the 42 changing; an output changing
  where a step says none does (the playback maps, grobs, layout, pages,
  traces, stderr but LilyPond's warnings a step names); a function of no
  part (`articulate.ly`'s Scheme among them); a C2b difference only
  libstdc++'s heap would give; past 2,500 lines at twice the top; the MIDI
  row over twice Must (reported) or T29's performance over twice with no
  lead (7); a device that cannot be reached at 9; D88 at the close.
- **Estimate:** 6,500–9,500 lines (README §5.1 after MS1); **≈ 4,450 at its
  split** (4,448: the ported lines' 3,611 at MS3's proxy top, 2,698, × MS3
  part 5's rate 1.352 = 3,648, the models 800; at MS5's own proxy 8,999, a
  bound). No step over 2,500 at twice its top (the largest, step 1's 2,422).
- **Exit:** **MS5's summary** above its step entries (IMPLEMENT's *Done for
  a run*); OPEN (G1 to R8, G2 and C2's R4 items closed) and LEDGER (next:
  R5's split, MS4 step 0, its first step the machinery on R1's lead).

## R5 — MS4: the whole corpus engraved (in parts)

**State:** **closed by MS4 step 23 (2026-10-27)**; split by MS4 step 0 (2026-10-25): steps 1–23 in five parts,
each ending with its close; **step 1 done (2026-10-25)**, the machinery on
R1's lead (OPEN D3: the grobs' pure Y extents and the breakers' line
heights in Rust): R1 on the 10 the iPhone 121.1 µs (1.01), the emulator
218.5 (1.82); its stop answered by the user: recorded; **steps 2–5 done
(2026-10-25)**, Y4: the beam engravers, beam.cc, the quanting, the stem and
chord tremolos, `Beam_collision_engraver`, and `Tweak_engraver` (in no
part: stopped, answered by the user: ported at step 4); T11 and T15 laid
out whole; **step 6 done (2026-10-25), part 1's close**: the engine's own
shape dump (`c5.tsv`), C5 0 on T11 and the beam fixtures, T15's layout
stderr LilyPond's, the devices measured, D7's memory split; the driver row
2.05 of its brotli Must, reported, **answered by the user: OPEN H4's retrace
and the row's attribution a session of their own, step 6a**; **step 6a done
(2026-10-25)**: H4 retraced (wasm-opt's inlining across crates into
`lily_step`, its bodies of no name), `sizes.mjs` by origin, judged so by the
user's answer (D225): the driver row 1.41 of its brotli Must, under twice;
**step 7 done (2026-10-25)**, Y5's slurs: the slur engravers and slur.cc but
the control points, the slurs fixture's grobs and each slur's stencil
LilyPond's, T22's TabVoice slurs left to part 4 (OPEN A44); **step 7a
done (2026-10-26)**, by the operator's ask: `Tab_note_heads_engraver` and
`Tab_staff_symbol_engraver` in `crates/tablature` (D223), pulled forward
from step 17, T22's grobs.tsv LilyPond's whole, its ten Slurs among them
(A44's TabVoice half closed; A46, a TabStaff's layout and the `fretted`
part, left to step 17); **step 8 done (2026-10-26)**, the operator's
answer when the run's plan gave the session the ties: slur-scoring.cc and
slur-configuration.cc, fixtures/slurs.ly and T27 LilyPond's whole with
C5 0; **step 9 done (2026-10-26)**: tie-formatting-problem.cc,
tie-configuration.cc, tie-specification.cc and tie-details.cc, the tie probe
on fixtures/ties.ly and T24 LilyPond's but `atan`'s last bit (stopped,
**answered by the operator: the `libm` crate's `atan` kept**, D226, OPEN
E21), nothing laid out changed; **step 10 done (2026-10-26)**: the ties,
tie columns, semi-ties and laissez vibrer ties drawn, fixtures/ties.ly,
T12, T24 and T29 LilyPond's whole, T23 in TRACED (A26), and
`Pitch_squash_engraver` pulled forward from R9 for T29's RhythmicStaffs
(stopped, **answered by the operator: pull it forward**); **step 11 done
(2026-10-26), part 2's close**: C5 on T12, T24, T29 and the slurs and ties
fixtures at p95 0 natively and in wasm, C3 and C4 of part 2's scores at
Δ 0, T27's and T29's stderr LilyPond's, the devices measured; D88 over
twice on the iPhone (T29 2.03) and the emulator (T12, T29, T24), the
translations holding whole layouts: **answered by the operator: recorded
(OPEN D8), MS4 step 21 cuts them**; part 2 5,889 lines (1.04 of 5,687),
MS4 ≈ 19,300 at part 2's rate 1.323; **steps 12 and 13 done
(2026-10-26, one commit)**: tuplet-engraver.cc, tuplet-bracket.cc,
tuplet-number.cc and `Drum_notes_engraver`, the tuplets and drums
fixtures, T07, T08, T09, T25, T26 and T28 LilyPond's whole (T25's links
as capture.ly reads them), 1,915 lines; **step 14 done (2026-10-26),
part 3's close**: C5 on part 3's scores at p95 0 natively and in wasm,
C3 and C4 at Δ 0, **C2.2 ported** (each compile's options reset after
it, as `lilypond-all`'s), the devices measured (the iPhone reconnected
by the operator's answer; D8's misses lower, recorded), part 3 1,925
lines (0.99 of 1,952), MS4 ≈ 19,400 at part 3's rate 1.363; **step 15
done (2026-10-26)**: the lyric engravers (`Lyric_engraver`,
`Extender_engraver`, `Hyphen_engraver`, the stanza numbers'), the
extender's and the hyphen's callbacks, fixtures/lyric-grobs.ly, the lyric
grobs of T02, T04, T20, T23 and T30 LilyPond's in grobs.tsv, T02, T04,
T23 and T30 laid out whole, A10 closed; T20's text **stopped (PROTOCOL
§6.6), answered by the operator: D224 widened (D227)**; 1,140 lines;
**step 16 done (2026-10-26)**: `crates/figured-bass` with the figured
bass engravers and the bracket's and continuation's callbacks,
fixtures/figures.ly, T06 and T21 laid out whole, E20 read again; 1,013
lines; **step 17 done (2026-10-27)**: `Fretboard_engraver` in
`crates/tablature`, `crates/chord-names`, A46 (D228: parts loaded before
the layout) and A47, fixtures/tablature.ly, T22 laid out whole; **stopped
twice, answered by the operator**: `ly:bezier-extent` ported, Guile's
arrays in the scheme crate (D229); 552 lines; **step 18 done
(2026-10-27), part 4's close**: C3 and C4 on all 30 but T20 at Δ 0 (T20
under D227), C5 on all nine at p95 0 natively and in wasm, B5b on T02
first measured (the iPhone 49 ms of 800), the devices measured (R1 on the
10 the iPhone 119.8, the emulator 202.6; D8's two as answered), the full
suite twice green; part 4 2,705 lines (0.95 of 2,843), MS4 ≈ 19,300 at
part 4's rate 1.311 with part 5's models 1,397; **step 19 done
(2026-10-27)**: W6's rest (runs by script and face, fallback through the
packs, Guile's names), the shaping check on the 52 references, E18
closed; 620 lines; **step 20 done (2026-10-27)**: K4's `wasm-split` of
`crates/tablature` and `crates/chord-names` (stopped, **answered by the
operator: the figured bass stays in the core, D230**), the gate in the
studio's corpus test, S6's `chords` and `fretted` as manifest parts; 100
lines; **step 21 done (2026-10-27)**: a kept pure child translation read
without the VM (T29's layout ×0.76 natively); 55 lines; **step 22 done
(2026-10-27)**: MS4's gate passed natively and in wasm, A25 and A28
reached by none (R9); **closed by step 23 (2026-10-27), MS4's close**:
the gate in two full runs, nothing skipped (787 passed; 272 of 272); the
devices (R1 on the 10 the iPhone 120.5, the emulator 212.8; B5b within
Goal; D88's emulator T12 2.51 and T29 2.88 over twice: **answered by the
operator: recorded, R7**); no code row nor the image over twice Must;
MS4 18,654 lines (0.99 of 18,838); MS4's summary in progress.md. **Next:
R6.**
In the main lane, after R4 and before R6
(PROTOCOL §5). The counts (`$LILY_CORPUS/ms4-split/`), the parts, the
order, the budgets, the stop points and the step lines **MS4 · steps 1–23**
are in progress.md's *MS4*, *The plan*.

**Settled with the user (2026-10-25;** in full in progress.md's *MS4*,
*Settled with the user at R5's split*; each the recommended option):

1. **All of MS4's step lines now, in five parts, each with its own close**
   (1 the machinery and beams; 2 slurs, phrasing slurs, ties; 3 tuplets and
   the rest of L5; 4 lyrics, figured bass, tablature, chord names; 5 W6's
   rest, K4, the budgets, MS4's close); each close re-estimates the parts
   left.
2. **K4's modules are three new crates, `tablature` (with fret diagrams),
   `figured-bass` and `chord-names`**, built as crates from their first step
   (16, 17); `ambitus`, `pedals` and `percent` stay in the core until R9
   (**D223**).
3. **K4's gate is the studio's `engine-corpus.test.ts`**, approved; with
   `modules.ts`' triggers, the only studio files R5 may change (steps 20,
   22); `protocol.ts`, `driver.ts`, `host.ts` stay a stop point (D223).
4. **W6's rest in the core; `OffscreenCanvas` at R8; T20's C4 under R11**:
   the grobs whose text no pack covers compared apart and recorded
   (**D224**).
5. **The devices at the machinery step and every close** (steps 1, 6, 11,
   14, 18, 23); D206 holds.

- **Builds (README §5, MS4):** Y4–Y6: beams with quanting, slurs, ties,
  tuplets, lyrics, figured bass, tablature; W6's rest (runs by script,
  fallback, the shaping check; D224); K4 (D223); S6's `fretted` and
  `chords` as manifest parts (I2). **Counted:** MS4's 45 `lily/*.cc` files
  16,095 lines; the engine has 381 (MS3's tie-engraver.cc, tie.cc,
  tie-column.cc, bezier.cc, bezier-bow.cc units); **15,683 left**, 15,581
  ported by steps 2–17; 20 callees of other files the engine lacks (288
  lines), each placed with its first caller; **no function of no part**.
  The census (`census.txt`): 14 engravers of the 30's grobs are
  placeholders; 45 of `PRIMS`' 72 open placeholders reached; each score's
  grobs all ported after: now the 10; part 1 + T11, T15; part 2 + T12,
  T24, T27, T29; part 3 + T07, T08, T09, T25, T26, T28; part 4 + T02, T04,
  T06, T20, T21, T22, T23, T30.
- **Carried into MS4 by earlier answers:** the slur, tie and beam
  engravers' grobs and warnings (steps 2, 7, 10), the layout half of T15's
  (6), T27's and T29's (11) and T26's (14) stderr (MS3 step 0's answer 1;
  D99's "compressing over-full page", T29's cyclic dependency);
  `Lyric_engraver`, `Extender_engraver`, `Hyphen_engraver` (D97, 15); the
  melisma translators (D98, 2, 7, 10, 15); `Tie_engraver`'s delegation
  (10); stem's beam and tremolo calls (A31: 3, 4, 5).
- **Inherits:** A10 (15), A19 (2, 3, 5, 7, 10, 11), A25 and A28 (22; A28
  also 14), A26 (10), A31 (3, 4), A41 (11, 14), B13 (11, 14), B14 (14),
  C2's R5 items (C2.1 18, C2.2 14 and 18, C2.5 2/7/10, C2.7 10, C2.8 and
  C2.9 15), D3 (1), D7 (6), E2 and E18 (19), E20 (16), I2 (20); C1, D1, D2,
  E4, E10, E13, H4 recorded at each close. **The triage** (MS4 step 0,
  `triage.txt`): C2's and F's other items R7 (F's include search ×3) or R9
  (17), F's `\breve` closed (MS5 step 3), C3 R9.
- **Steps** (MS4 · 1–23; progress.md's *MS4*, each with PLAN-RUN §5's five
  fields): part 1: **1 the machinery on R1's lead** (R1 on the 10 with the
  devices; a stop if still over 120 µs on D206's two), 2 the Beam grobs, 3
  beam.cc, 4 quanting, 5 tremolos and collisions, 6 part 1's close (the
  engine's C5 dump, C5 on T11); part 2: 7 slurs, 8 slur scoring, 9 tie
  formatting, 10 ties (A26: `TRACED` 30), 11 part 2's close; part 3: 12
  tuplets and brackets, 13 tuplet numbers and drums, 14 part 3's close;
  part 4: 15 lyrics, 16 figured bass's crate, 17 tablature's and chord
  names' crates, 18 part 4's close (all 30 laid out); part 5: 19 W6's rest,
  20 K4, 21 the budgets, 22 **MS4's gate**, 23 MS4's close.
- **Gate:** C3 and C4 on all 30 (T20 under R11, D224); **C5** on the nine
  with beams or slurs (T08, T11, T12, T23–T26, T29, T30), Must ≤ 0.25 staff
  spaces at p95; each of the 30 compiles with only its triggered modules and
  no restart, and a module left out gives a restart, not a different result
  (`engine-corpus.test.ts`, D223); natively and in wasm (step 22; both full
  runs at 23). Each part's close holds its own scores to C3, C4 and C5.
- **Budgets:** R1 on the 10 (the iPhone 146.5 µs, the emulator 259.5 at
  MS5 step 9) at step 1 and every close; B5b on T02 (Android 800 / 400 ms)
  first at 18, cut at 21; whole-score layout (S 150 / 50, L 500 / 200 ms) at
  each close, cut at 21; the core's size against §4.1, every crate within
  its share, each notation module ≤ 100 / 40 KB brotli (05 §9); D88's
  translations at each close; memory split at 6 (D7).
- **Stop points:** R1 still over 120 µs on the iPhone or the emulator after
  step 1; a crate beyond D223's three, or one of them needing more than
  layout and music; the music crate naming a layout type; `protocol.ts`,
  `driver.ts`, `host.ts` or a studio test other than `engine-corpus.test.ts`
  changing; a function of no part; a reference of the 30 changing; an
  output changing where a step says none does; a C5 deviation over Must
  with no cause; a score failing C4 only where its Latin, Greek or Cyrillic
  text is placed (D224 keeps only text no pack covers as R11's); past 2,500
  lines at twice the top; B5b, a whole-score layout or a module over twice
  its Must after step 21's cuts with no lead; a device that cannot be
  reached at 1, 6, 11, 14, 18 or 23; D88 at each close.
- **Estimate:** 26,500–32,500 lines (README §5.1 after MS1); **≈ 18,800 at
  its split** (18,838: the ported lines' 15,869 at MS3's proxy top, 11,856,
  × MS3 part 5's rate 1.352 = 16,030, the models 2,808; MS5's 1.032 and
  MS3 part 4's 1.50 give 15,043 and 20,591, the bounds). By part: 1 6,959;
  2 5,687; 3 1,952; 4 2,843; 5 1,397. No step over 2,500 at twice its top
  (the largest, step 2's 2,452).
- **Exit:** MS4's summary above its step entries; OPEN (R5's items closed)
  and LEDGER (next: **R6**, the merge into `main`, which asks the user
  first).

## R6 — Merge: the engine joins main (D202)

**State:** **closed** (2026-10-27, in six sessions; LEDGER.md's *Next
action* and *History*). The operator's answers at session 1: `merge/r6`
from `origin/main` 2e11da3 in `.worktrees/r6-merge`, `feat/studio-web`
first as a real merge (`main`'s side for the overlap it shares with PR
#2), the engine renumbered on `merge/r6-renumber` (`.worktrees/r6-renumber`)
and merged after it, `main` fast-forwarded and pushed at the close, no
force push, no squash. Done: `feat/studio-web` merged (`8efcb59`), **the
engine's D113–D119 renumbered D126–D132 (`587577c`, D231; `main` had
moved to D125, so not D124–D130)**, the engine merged (`1bd1aa0`: three
conflicts, Cargo.toml, AGENTS.md with six packages, DECISIONS.md in
number order); every suite green at `1bd1aa0` and on `main`; `main` and
`feat/engine` fast-forwarded to the close and pushed. No resolution
changed behaviour, no stop reached, the main checkout untouched. **After
the merge** (the operator's answer at the close): later plans work on
`feat/engine` fast-forwarded to `main`, in `.worktrees/engine`. The
steps as planned:

- **Steps (its split refines them):**
  1. Inventory: `main` vs `feat/studio-web` (D55's web work, D112–D115 on
     it, IMPLEMENT.md and PLAN-RUN.md) vs `feat/engine`; the uncommitted
     work in the main checkout (today `packages/studio/website/`); the
     user confirms the order and the target branch.
  2. `feat/studio-web` into `main` (or its rebase), the studio's tests.
  3. `feat/engine` into `main`: `packages/studio/Cargo.toml` members and
     lock, AGENTS.md's package list (four packages, D53, D56),
     `.gitignore`, and **DECISIONS.md: the engine's D113–D119 renumbered
     after `main`'s last**, every citation in progress.md, the runway, the
     chapters and code comments following; D200 and up stay.
  4. The full suites of every package (`npm test` at the root, the
     engine's two runs, the studio's), nothing skipped.
  5. Close: LEDGER names the branch R7 works on (from `main`), and the
     worktree convention after the merge.
- **Stop points:** any conflict whose resolution changes behaviour;
  pushing or opening a PR (only when the user asks).

## R7 — MS6: only what an edit changes

**State:** **closed (MS6 step 13, 2026-11-01):** the differential passed, 912 of 912 natively and in
wasm at steps 12 and 13; B6 recorded as a miss, B8 on 26 pages R8's, R8's first step translation's cost (D256); the earlier
gates in both full runs; 16,242 lines against ≈ 14,000 (MS6's summary in progress.md). **Next: R8's split.** Split by
MS6 step 0 (2026-10-27): steps 1–13 in five parts,
each part's close folded into its last step (3, 7, 9, 11). On `feat/engine` (fast-forwarded to `main` at
R6's close), in `.worktrees/engine`. The counts
(`$LILY_CORPUS/ms6-split/`), the parts, the order, the budgets, the stop
points and the step lines **MS6 · steps 1–13** are in progress.md's *MS6*,
*The plan*.

**Settled with the user (2026-10-27;** in full in progress.md's *MS6*,
*Settled with the user at R7's split*):

1. **All of MS6's step lines now, in five parts, each ending with its
   close** (1 the harness, S5, the re-parse; 2 N8; 3 Y7; 4 W4, W8, H3; 5
   the gate and the close); each close re-estimates the parts left.
2. **The differential harness first, with seeded typed edits** (a pitch,
   a duration, a token inserted or deleted, a string's character, a bar
   deleted, and edits that make an error), about 10 a score of the 30 in
   `test:tools`, a larger seeded run at each part's close and the gate.
3. **W8 in R7: one message in `protocol.ts`** (a page's cells replaced by
   digest), carried by `worker.ts` and passed on by `driver.ts` and
   `host.ts`, handled in `packages/common/web/preview.js` and the studio's
   renderer, with tests; the studio's page (H2) stays R8's (**D232**).
4. **B8's score is T29's music repeated** by a kept script: LilyPond 2.26.0
   lays out T29 × 26 on 26 pages (× 8 on 8), so T29 × 26.
5. **The devices only at MS6's gate and close** (steps 12, 13): B6 judged
   on the iPhone and the emulator, the Android tablet beside
   (**D233**, as D206 and D214).
6. **OPEN D8 recorded at each close; a step of its own only if B6's
   translation phase misses Must after N8.**

- **Builds (README §5, MS6):** S5 (the write barrier and undo log turned
  on, dependency tracking); 02 §5.7's re-parse; N8: checkpoints and
  convergence, then partitions and replay; Y7: digests, cells, the line and
  page DPs updated, 05 §7's queue with sub-steps; W4: markup kept per grob
  and cell; W8: the cell-replacing message in the preview (D232); H3: the
  engine returns between systems. **Counted** (`design.txt`,
  `engine.txt`): no LilyPond file; the chapters' design for the eleven
  code steps 307 lines (S5 21, the re-parse 40, N8 96, Y7 77, W4 and W8
  28, H3 24, the harness 21); the engine at 8ca7c5a has S1's epochs, S3's
  slot versions, N2's digests and C2.2's reset, and lacks checkpoints, an
  undo log and cells; one step a compile. Cells 133 over the 30 (T29 28,
  T24 18, T02 9, T12 5); E1–E3 found on the four; the typed edits'
  population 1,864 note-name links, 79 durations, 412 strings.
- **Inherits:** E11 (step 10); F's three include gaps (3); C2.2's
  persistence across compiles (2, with the undo log); R1's translation half
  (T24's reflow, 7) and layout half (T29's reflow, 9); D1's misses and D8
  (12, 13; answer 6); C1, D2, D7, E4, E10, E13, H4 recorded at the parts'
  closes and the close.
- **Steps** (MS6 · 1–13; progress.md's *MS6*, each with PLAN-RUN §5's five
  fields): part 1: **1 the differential harness**, 2 S5, 3 the re-parse and
  F's include gaps (part 1's close, C0 incremental); part 2: 4 N8's
  checkpoints, 5 convergence and the change list (C1, C2), 6 partitions,
  7 replay and T24's reflow (part 2's close); part 3: 8 Y7's cells (C3–C5),
  9 the DPs, the queue and T29's reflow (part 3's close); part 4: 10 W4 and
  W8, 11 H3, B8 and memory (part 4's close); part 5: 12 **MS6's gate** with
  the devices and **B6's decision**, 13 MS6's close.
- **Gate:** an incremental compile equals one from scratch on 00's edits
  E1–E3 and on the seeded typed edits of the 30, in every layer C0–C5,
  natively and in one `core.wasm` instance; **B6** on T02, T12, T24 and T29
  with E1–E3, 20 each, on the iPhone and the emulator (the tablet beside,
  D233), phase by phase against README §4.3; B8 with T29 × 26 (26 pages);
  memory with the old and new run against §4.5.
- **Budgets:** B6 by phase (S 250 / 100, L 500 / 200 ms; never measured);
  parse, translation, layout, SVG and the page per edit at the parts'
  closes natively and on the desktop; the longest step between checks
  (20 / 5 ms) at 9 and 11; B8 (50 / 20 ms) at 11 and 12; memory (S 64, L
  128 MB Must) at 7, 9, 11, 12; D88's translations (OPEN D8: the emulator's
  T12 2.51 and T29 2.88 of Must at MS4's close), R1, the first restore and
  sizes at 12 and 13.
- **Decision point:** **B6** (README §5, IMPLEMENT's *Limits*): step 12
  reports it phase by phase on the iPhone and the emulator, and the user
  decides.
- **Stop points:** B6's decision point (12); a difference the harness finds
  with no cause (from 2), or a planted difference it does not catch (1);
  the music crate naming a layout type; a crate beyond today's;
  `protocol.ts`, `driver.ts`, `host.ts` or a studio test changing beyond
  D232 (10); a from-scratch output or a reference changing; a function of
  no part; past 2,500 lines at twice the top; a per-edit phase, B8 or
  memory over twice its Must with no lead; B6's translation phase over
  Must after N8 (D8); a device that cannot be reached at 12 or 13; D88 at
  the close.
- **Estimate:** 15,500–24,500 lines (README §5.1 after MS1); **≈ 14,000 at
  its split** (14,045: README's 7,000–9,000 apportioned by design share,
  the top × MS4's rate against its chapter figure, 1.555 = 18,654 /
  12,000; the bottom 10,966, at README's 2.7 24,350). By part: 1 3,738;
  2 4,375; 3 3,510; 4 2,370; 5 52. No step over 2,500 at twice its top
  (the largest, steps 3 and 8's 2,345) after N8 was cut in four.
- **Exit:** MS6's summary above its step entries; OPEN (R7's items closed)
  and LEDGER (next: **R8's split**).

## R8 — MS7: in Lily Studio on the web

**State:** **closed (MS7 step 11, 2026-11-04): MS7's gate passed but for the default (D265):** D55's tests with the engine and
refused, Chromium and WebKit end to end, a second visit asking only the manifest, the network dropped, no trap, no killed tab on
the stand-in and the fallback: passed; B1–B6 measured and recorded; **B7 on the tablet and B8 through the page failed on T29
alone, hard items, and the source is still private, so the engine is not the web's default.** **The full runs: run 1 all green;
run 2 (the larger differential) green but `test:tools` 301 of 305:** the four typed-edit tests at 32 edits a score fail on T29's
`type-21-pitch` and `type-22-again` (OPEN B29) and T26's `type-31-pitch` for a preview (B32), with N8's checks on four more typed
edits behind them (C15). **R9's first step fixes those, with run 2's size in its suite; then the default.** 5,637 lines against 4,500–8,000 and the split's ≈ 11,800 (MS7's summary in progress.md). **Next: R9's split.** Split by
MS7 step 0 (2026-11-01), in `.worktrees/engine` on `feat/engine`, on `main`'s line. Steps 1–11 in four parts. The engine starts shipping here. The counts
(`$LILY_CORPUS/ms7-split/`), the parts, the order, the budgets, the stop points and the step lines **MS7 · steps 1–11** are in
progress.md's *MS7*, *The plan*.

**Settled with the user (2026-11-01;** in full in progress.md's *MS7*, *Settled with the user at R8's split*):

1. **H2/D58 ported into `ScorePreview`:** its own one-page-a-frame loop, the visible page first, with `insertionOrder` and
   `pageRatio` from `preview.js`, which does not change. Step 9.
2. **The Android floor: the Android e-reader as a stand-in** (**D257**), said so at the gate; OPEN D4 stays open for a real phone.
3. **E14: all outlines on demand:** metrics in the pack, the outlines fetched when a skyline first needs one. Step 5. B1 read
   both ways, 0.994 to 1.046 of Must, as T02's own text may reach them.
4. **The source published, then ship:** before step 9 the operator makes the engine's source public at each shipped commit
   (OPEN I5: `velet5/lily` is private); the page links it; the site carries the GPL, URW's licence with its exception and the
   fonts' source. Work behind `?engine=1` goes on until then.
5. **The budgets measured, with hard items** (**D257**): the default needs no trap, no killed tab on the stand-in, B8 and B7
   within Must, and the fallback working; B1–B6 are measured and recorded, a miss over twice stopping at step 10 (D88).

- **Builds (README §5, MS7; 08 §7):** K5: `Cache-Control` in D55's gateway, `persist()` after use, the debug build; K6:
  `engine.js`, live preview in the page, the server's live compiles off while the engine runs, the server as fallback for every
  compile the engine fails or cannot do (`?engine=1` first); M7: the performance after the page, cancellable, Export MIDI from
  the engine's bytes; K7: 00 §6's full measurement on the reference devices; H2 / D58's port to `ScorePreview`. **Counted**
  (`design.txt`, `engine.txt`): what is left to build 111 design lines (K5 16, K6 39 + 18, M7 23, H2 15). The tree at a0d1141 has
  `EngineHost.start`, `set_live`, the `--site` build, the CSP, `PageCells` and the outcome's MIDI; it lacks `engine.js`,
  `?engine=1`, `Cache-Control`, `persist()`, `core-debug`, `OffscreenCanvas`, a web font and a commit in the manifest.
- **Inherits** (each on its step, `open.txt`): C11, C13, D8 (1); C15, C16 (2); B18, B21, B22, D3 (3); B20, B23, B24 (4); E14, E15,
  D1's room in B1 (5); C5, E2, E17, E22, I2, I4, F's include search (7); G1 (8); I3, I5 (9); C1, D2, D4, D5, G3 (10); I1 (11).
- **Steps** (MS7 · 1–11; progress.md's *MS7*, each with PLAN-RUN §5's five fields): part 1, the engine's cost: **1 translation's
  cost per edit**, 2 shared snapshots and the 26-page trap, 3 layout per edit, 4 the steps a newer edit waits on; part 2: 5 room
  in B1, the licences and K5; part 3, the page: 6 live preview behind `?engine=1`, 7 the fallback and the host's gaps, 8 M7 and
  G1, 9 D58's port and **the default**; part 4: 10 K7 on the devices, 11 MS7's close.
- **Gate (as D257 reads it):** D55's tests with the engine and with the engine refused; the page driven end to end in Chromium
  and WebKit; a second visit downloads only the manifest; rendering goes on when the network drops. **Hard:** no trap, no killed
  tab on the stand-in, B8 and B7 within Must, the fallback working. **Measured and recorded:** B1–B6 on the iOS and Android
  references (the iPhone and the emulator deciding, D206, D233), B4 on the stand-in.
- **Budgets:** progress.md's *Budgets at the split*: B1 1.07, the text packs up to 2.6, B4 T29 1.47, B6 up to 2.20 (T29's
  translation 11.6), B7 2.45 on the tablet, B8 within Must on the four, T29 × 26 a trap, D88 over twice on T12, T24, T29, R1.
- **Stop points:** a hard item failing (9, 10); D88 over twice among B1–B6 (10); the source not public (9); the fonts' licence
  text differing from D56 and D209 (5); T02's first render or compile over twice through answer 3's fetch (5); a difference
  the harness finds with no cause (1–4); an output changing where a step says none does (1–5); `protocol.ts`, `driver.ts`,
  `host.ts` and the studio's page may change, **each with a decision**; the Rust compiler or error parser changing; a function
  of no part; past 2,500 lines at twice the top; a device that cannot be reached (10).
- **Estimate:** 4,500–8,000 lines (README §5.1 after MS1, its MS7 alone); **≈ 11,800 at its split** (11,815): README's pieces
  5,415 (2,000–3,000 by design share, the top × MS6's rate 1.805 = 16,242 / 9,000; the bottom 3,610), what D256 moved here and
  the host's gaps 6,400 at their models. By part: 1 4,934; 2 1,127; 3 5,702; 4 52. No step over 2,500 at twice its top (the
  largest step 7's 2,155). Lines count the studio's code beside the engine's.
- **Exit:** MS7's summary above its step entries; OPEN (R8's items closed or re-owned, D4 open for a real floor) and LEDGER
  (next: **R9's split**).

## R9 — MS8: beyond the corpus

**State:** **closed (MS8 step 12, 2026-11-12), R9's close and the grand plan's end: MS8's gate passed** (PDF, Export PDF on the web, Y01–Y90 at C0–C5) in both full runs, step 11a's suite at run 2's size on the close's code; the devices through the studio's page: **B8 on T29 within Must on all three, B7 on T29 failing on Android, recorded as at R8 (D265) by the operator's answer**, no trap, no killed tab on the e-reader; the source private, so **the default is not turned** (answer 4); 17,674 lines against ≈ 21,800 (0.81); **every OPEN item past the runway**. Before it: split (MS8 step 0, 2026-11-05); steps 1 and 2 done (2026-11-05, D266, D267, D176); step 3a added after step 2 by the operator's answer (B23's `\score` item and `create_top`, B28's read, before step 4, every output LilyPond's to the bit); step 3 done (2026-11-06, D268); step 4 done (2026-11-06, D269, D177: I7, the visible page handed over before the compile's end; I6, I8, E22, G5 not taken up, carried to step 11), run before step 3a by the operator's answer at its stop, with the source still private and no `--engine-default` in `build:site`; steps 5 and 6 done (2026-11-06, 2026-11-07, D270, D271, D141); step 7's part 1 done (2026-11-08); step 7's rest of part 1 and its part 2 done (2026-11-08, no decision: `percent` kept in the core and A44 moved to step 8, the operator's answers at part 2); step 8's part 1 done (2026-11-09, no decision: the piano pedals kept in the core, the operator's answer; A19's trill line built; A44's completion engravers left to part 2); step 8's part 2 done (2026-11-09, no decision: ambitus kept in the core, the operator's answer; A44 closed): step 8 done. Step 3a done (2026-11-09, D272: a `\score`'s `scorify-music` in the VM's steps, create_top's lists asked ahead, T29's line of marks held with LilyPond's bits (B28 closed), a timestep's parts; **the operator's answers at its stop: B8 on T29 itself, held in core.wasm; T29 × 8 and × 26 left in B23 past R9**). Step 9's part 1 done (2026-11-10, no decision: the ligatures' base, the ligature bracket, mensural ligatures and the episema, Y41–Y44; the episema's score moved to part 2). Step 9's part 2 done (2026-11-10–11, no decision: the Vaticana and Kievan ligatures, A38's divisiones (A38 closed) and custodes, Y45–Y50 with `episema.ly`; **the operator's answer at its §6.3 stop: `ly:engraver-make-item` ported in the part**). Step 9's part 3 done (2026-11-11, no decision: the five other page breakers and `Page_turn_engraver`, Y51–Y64; Y51, Y54 and Y55 judged as T20 under D227): **step 9 done**. Step 10 done (2026-11-11, no decision: `Quote_iterator`, OPEN C4 closed (stale since MS5 step 6), P12 and OPEN F's R9 gaps closed but `write` of music, which is C6's (step 11), Y65–Y83 and `p12.test.mjs`). Step 11 done in one session (2026-11-11, **D273**: syntax-case a split module on demand, 5,471 brotli of §4.2's 40 / 20 KB, `(lily accreg)` loaded on use; **D274**: LilyPond's own Scheme printed as compiled, the music types' `display-methods` before the `display` part; S6's `qr` and `modal` reached, Y84–Y90; OPEN C3, C6 and C8 closed, C2.3 and C2.10 done, C2.4 not a defect, C2.6 stale, C23 opened; C22 scanned again). **The operator's answer at step 11's push (2026-11-11): A13, A25, A28, B18, B19, B22, C16, C17, C19–C21 and C23 are carried past R9, past the runway, without work; C23's per-staff timing difference has no cause found. D274's image cost trimmed first** (formals kept for the image's toplevel procedures only; every output equal; the suite at run 2's size again), then step 12, its devices one at a time (one cable). **The operator's answer at step 12 (2026-11-12): B33 fixed first, step 11a (D275: a layout's error ends the run as a translation's), the suite at run 2's size, pushed; E22, G5, I6, I8, I9, C15's two, H15's full sweep and C2.1 and C2.9 carried past R9, past the runway, without work.** **The order from there, the operator's answer (2026-11-08): the plan's order:** step 7's rest (coda and segno marks and section labels scored, `Jump_engraver`, A37; part 2 with the `percent` module question asked when reached), then step 8, then step 3a, then steps 9, 10, 11 and 12, each finished before the next starts, nothing left past R9's close.** In `.worktrees/engine` on `feat/engine`, on `main`'s line,
lane A (answer 1: lane C not opened). Steps 1–12 in four parts, **17 sessions' parts**, **≈ 21,800 lines**. The counts
(`$LILY_CORPUS/ms8-split/`), the parts, the order, the budgets, the stop points and the step lines **MS8 · steps 1–12** are in
progress.md's *MS8*, *The plan*.

**Settled with the user (2026-11-05;** in full in progress.md's *MS8*, *Settled with the user at R9's split*):

1. **Lanes: the main lane;** W7 at steps 5–6, no decision block.
2. **E2 (`OffscreenCanvas` for T20's text): left past R9;** no ABI growth for text widths; T20 under D227.
3. **I7 (the visible page before the compile's end): built at step 4,** the ABI's output before `Step.Done` with a decision.
4. **I5: the source stays private,** so the default stays off (D265): step 4 does everything but `--engine-default` in
   `build:site`, and R9 closes with the default recorded, as R8 did. **The grand plan's *Done when* (the engine the web's
   default) cannot be met as written while it stays private.**
5. **W7's licence: the URW fonts in PDFs, as LilyPond's,** subset under URW's PDF exception, shipped unmodified with their
   licence and a link to their source; step 5 reads the licences again first.
6. **The A15-class phone: the Android e-reader stands in again** (D257); OPEN D4 stays open past the runway.

- **Builds (README §5, MS8), as counted at the split:** the default's items first (D265); W7, the PDF module, and Export PDF on
  the web; **Y8: the 83 grob types no score reaches, 50 of them lacking a maker or callback** (3 of the five, 5 of ancient
  notation's, 42 more: pedals, percent repeats, trills, ambitus, clusters, cue clefs, balloons, grids …), **10,125 LilyPond
  lines left with the quote iterator**; **the iterators: only `Quote_iterator` is left** (the part combiner, percent repeats
  and `\fine` are MS5's), with C4 read again; P12 and F's gaps; **S6: all seven parts built as modules**, `modal` and `qr`
  reached by no score; `syntax-case` (refused by D60's expander today). Each feature with a new test score from
  `input/regression/` covering something none of the others does.
- **Inherits** (each on its step, `open.txt`; 49 items naming R9): B29, B32, C15 (1); B23, B28, I3 (2); B25–B27, B30, B31, C18,
  H15 (3); I5–I8, E22, G5 (4); E5 (5); E23 (6); A37, A44 (7); A19 (8); A38 (9); C4, F's 12 (10); A13, A25, A28, B18, B19, B22,
  C2, C3, C6, C8, C16, C17, C19–C22 (11); D3, D8, H16 (12); past the runway E2, D4, D9, G6.
- **Steps** (MS8 · 1–12; progress.md's *MS8*, each with PLAN-RUN §5's five fields): part 1, the default's items: **1 run 2's
  typed edits**, 2 T29's parse and a page's rendering, 3 the soundness items, 4 I7 and the default's gaps (no flag); part 2,
  PDF: 5 W7's module after the licence check, 6 Export PDF and E23; part 3, the long tail: 7 Y8's five, the marks and the
  repeats (2 parts), 8 Y8's common rest (2), 9 ancient notation and the page breakers (3), 10 the quote iterator, `\fine` and
  P12, 11 S6, `syntax-case` and the tail (2); part 4: 12 MS8's gate, the devices and the grand plan's close.
- **Gate:** for PDF: page count, rasterised against the engine's own SVG with pdf.js, lyrics and titles as text, the module
  within §4.2, the file within 150 % of LilyPond's (`pdf.txt`: T02 84,798, T20 187,455, T24 339,729, T29 471,931 bytes); then
  Export PDF on the web from the engine. For each new score: C0–C5 as for the 30. **The default stays as D265 left it** while
  the source is private (answer 4).
- **Budgets:** progress.md's *Budgets at the split*: B1 1.005, B3 2,974,867, the PDF module none yet (500 / 100 KB), B6 through
  the page up to 6.22 of iOS Must, B7 the tablet T29 63–116 ms and B8 T29 28–190 ms (hard), D88 and R1 as R8's close.
- **Stop points:** the source (4, 12; the flag asked, never assumed); the core's ABI (4, 6: I7's output and the PDF call, each
  with a decision; any other growth stops); the licences (5); module boundaries (7, 8: `percent`, `pedals` and `ambitus` as
  modules on demand or the core, asked with their sizes); a new regression score's C0–C5 with no cause (7–11); an output
  changing where none should, with run 2's size in every suite (1–11); the hard items (2, 12); text widths; a function of no
  part; past 2,500 lines at twice the top; `protocol.ts`, `driver.ts`, `host.ts` and the studio's page each with a decision;
  the Rust compiler or error parser; the devices (12).
- **Estimate:** 37,500–50,000 lines (README §5.1 after MS1, before MS3–MS7 ported part of what it counted); **≈ 21,800 at its
  split** (21,769): ported 10,642 at MS4's rate 1.051 a LilyPond line, modelled 11,127. By part: 1 4,182; 2 2,792; 3 14,743;
  4 52. No part over 2,500 at twice its top (9.2's 2,397 the nearest). Lines count the studio's code beside the engine's.
- **Exit:** MS8's summary above its step entries; OPEN (R9's items closed or re-owned past the runway); LEDGER (**the grand plan
  done**, the default's state recorded).
- **After the runway:** R10's LilyPond 2.28 upgrade, as a milestone of its own (README §8).
