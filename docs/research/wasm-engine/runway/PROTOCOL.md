# How the runway is run

The rule books still apply in full: PLAN-RUN.md §2–§4 (the loop, the step
contract, progress.md's entry format) and IMPLEMENT.md's *Where to work*,
*Limits* and *Rules* (both on `feat/studio-web` until R6). This file adds
only what coordinating several plans needs.

## 1. The cycle

```
LEDGER says "next: Rk, step n"
  └─ the driver prompt (§2) starts a Mother plan for Rk's next steps
       └─ for each step: a step prompt (§3) → one or more sessions
            └─ each session: notes, handoff; the step's last session: progress.md entry
  └─ Rk's closing step: gate, devices, sizes, lines, OPEN.md, LEDGER.md
LEDGER says "next: Rk+1, split" (or "waiting for the user: …")
```

- **A plan's split step** (the first step of R2–R9; R1's is done) gets
  the user's answers to the plan's questions first (as MS3 step 24's
  "Settled with the user"), counts the files and LilyPond lines with kept
  scripts (`$LILY_CORPUS/<msN-partK>-split/`, as `ms3-part3-split/`),
  orders the steps by what each waits for, re-estimates the lines at the
  previous part's rate (R12), cuts any step over a session's 2,500 lines
  at twice its top, writes the plan's stop points and the step lines into
  progress.md under the milestone's *The plan* (PLAN-RUN §5's five fields:
  title | scope | files | minutes | why), and updates PLANS.md's section
  and LEDGER.md. A milestone's first plan also writes the milestone's
  *The plan* (as MS3 step 0 did).
- **A step** is run as in MS3 steps 25–35: its own Mother plan when it
  needs more than one session (one session a plan step, `Session k of n`,
  each ending with a commit and a `sessionK-handoff.md`), the last session
  running the full suite and writing the progress.md entry.
- **A plan's closing step** runs the plan's gate with nothing skipped,
  the earlier gates, the devices, sizes against the last close, the lines
  against the estimate (R12), D88's and R2's reports, and then updates
  OPEN.md (closed items struck with the step, new items added with an
  owner) and LEDGER.md (state, budgets, next action). A milestone's last
  plan also writes the milestone's summary above its step entries
  (IMPLEMENT's *Done for a run*), as MS0–MS2's.

## 2. The driver prompt (one Mother plan per runway plan)

Give this to a new chat, with the folder `docs/research/wasm-engine/runway/`
(in `.worktrees/engine`), to run the plan LEDGER.md names next. The same
prompt is used for every plan; LEDGER.md says which.

```text
RUNWAY — run the engine's next plan

You plan and run one plan of the engine's runway: the coordinated set of
plans that finishes our LilyPond engine in WebAssembly (MS3 to MS8).

Read, in the worktree .worktrees/engine (branch feat/engine), in order:
1. docs/research/wasm-engine/runway/LEDGER.md: where the runway stands
   and the next action (the plan, its next step, or a question waiting
   for the user).
2. runway/README.md, runway/PROTOCOL.md (this cycle, the step prompt's
   template §3, the stop points §6), the plan's section of
   runway/PLANS.md, and the plan's items in runway/OPEN.md.
3. PLAN-RUN.md §2–§4 and IMPLEMENT.md's "Where to work", "Limits" and
   "Rules" (`git show feat/studio-web:docs/research/wasm-engine/…` until
   the merge plan R6 brings them to main).
4. docs/research/wasm-engine/progress.md from the end: the last entry's
   Next and Open are the handoff. If its Next does not match LEDGER.md's
   next action, stop and report.

Then:
- If LEDGER.md's next action is a question for the user, ask it (with
  the options and the recommendation the ledger gives) and stop.
- If the plan has no step lines yet, its first step is its split
  (PROTOCOL.md §1): plan that step alone, and stop after it.
- Otherwise split the work into sequential steps, one per step line of
  the plan from the next one on, up to and including the plan's closing
  step. Each step's prompt follows PROTOCOL.md §3's template, filled from
  the step line in progress.md, the previous entry, PLANS.md and
  OPEN.md: its scope, what is already in the engine to call rather than
  port again, the guesses to check, its stop points, its budgets with
  the last figures, its line estimate and how to count them, the
  commands, and where to keep session notes.
- A step that cannot fit one session says so in its prompt: its sessions
  finish whole parts, commit, and hand off; its last session runs the
  full suite and writes the progress.md entry.
- Work only in .worktrees/engine on feat/engine (never the main checkout)
  until LEDGER.md says R6 has merged; then on the branch LEDGER.md names.
- New decisions take the next free number at D200 and up (D200).
- The plan's closing step updates runway/LEDGER.md and runway/OPEN.md.

Do only this plan. When its closing step is done, or a step stops for
the user, the run ends; LEDGER.md then names what comes next.
```

## 3. The step prompt's template

Every step prompt follows the shape of MS3 steps 35 and 36's (progress.md;
the user's chat history keeps them), in this order:

```text
STEP <k> — <title>

You are running one step of a plan run that builds our LilyPond engine
in WebAssembly. Your step is the line below: its title, scope and files.

Scope: <the step line's scope; its rows of the split (files, functions,
lines) and the plan's line in progress.md to grep; the part notes to read>.

Already in the engine, so call or extend these instead of porting them
again: <crates, files, preludes and primitives the step builds on>.

<n> guesses, to check before relying on them: <each with what to do if
it is wrong: stop and ask, port it here under Deviations, or record>.

Expected files (add what the port needs and list it under Deviations):
<the step line's files>.

Read first: PLAN-RUN.md §3–§4, IMPLEMENT.md's "Where to work", "Limits"
and "Rules" (on feat/studio-web until R6), runway/LEDGER.md and the
plan's section of runway/PLANS.md; then progress.md from the end.

Work only in the worktree .worktrees/engine, on branch feat/engine.
Never edit, stage or commit anything in the main checkout.

Do only your step. … (PLAN-RUN §3's paragraph, unchanged)

Stop and report, without guessing, when: <PLAN-RUN §3's three, the
plan's own stop points (runway §6 and PLANS.md), and the step's>.

Done means: <PLAN-RUN §3's five, unchanged>.

Where things stand (<date>): <HEAD, the last entry, what compiles how
far, what the step changes for the first time>.

Not this step's business: <the neighbouring steps' and plans' items>.

Budgets at <the last measurement> (<file and directory>): <core.wasm,
code and data sections, the rows the step touches, core.image, the
download, PRIMS, HOSTS, per-edit compile time with its script>.

Lines: <the split's estimate, twice, the re-estimate; how to count
(`git diff <base>` over `crates/*/src/*`, non-blank added and removed,
added code lines); what to count apart>.

The next free D-number is D<n> (D200 and up).

Commands: npm run build first; npm run test:rust and npm run test:tools as
two background runs with the longest timeout (the last totals); a test
that times out under load rerun alone; a regression the full run finds
fixed, rebuilt and both runs repeated; npm run lint:rust after the run;
the root's npm run lint; cargo +stable natively (D57).

Keep session notes in $LILY_CORPUS/device-runs/<date of the first
session>-ms<N>-step<k>/NOTES.md, one section a session, with a
sessionN-handoff.md each.
```

## 4. What each plan's closing step leaves

1. The plan's gate (PLANS.md) with nothing skipped, both full runs counted.
2. The earlier gates still passing: part 1's (the 11 pages PAE ≤ 1),
   `grobs.tsv` of the 10, the traces of `TRACED`, MS2's C1 on the 30,
   C0, b1–b8 and the function probe, and every later plan's gate.
3. Devices: desktop Chromium and WebKit, the iOS 26.3 Simulator, the Android
   tablet (or the emulator `mother_api36` where the plan allows), the
   real phones listed by hand.
4. Sizes (`scripts/sizes.mjs`) against the last close; R2: a code row (by
   origin, D225) or the image over twice its Must reported to the user before the next
   plan is split.
5. D88: each device miss over twice, at its median, listed with its lead
   for the next plan's split.
6. Lines against the plan's estimate (R12), its machinery and borrowed
   lines apart.
7. OPEN.md and LEDGER.md updated; the session notes' directory named.

## 5. Lanes

The runway has one main lane (R1, R2, R3, R5, R6, R7, R8) and two that
README §5.1 allows beside it:

- **Lane B, MS5 (R4)**: needs only MS2. **Default: run it in the main
  lane between R3 and R5** (it is small, and MS4's MIDI-side items, D103's
  playback map and the performers, then find MIDI done). Run beside the
  main lane only if the user opens a second worktree,
  `.worktrees/engine-midi` on `feat/engine-midi` from `feat/engine`
  (never a sibling directory), and then: R4 touches only `crates/smf`,
  the performers in `crates/music` and their tests; it rebases on
  `feat/engine` at each of its step's ends; it is merged into `feat/engine`
  before R6.
- **Lane C, W7 (PDF, part of R9)**: may start after R3 (MS3's W3), under
  the same rules, in `.worktrees/engine-pdf` on `feat/engine-pdf`.

Two lanes never take decision numbers at the same time without the
ledger: a lane's split reserves a block (e.g. D240–D259) in LEDGER.md.

## 6. Stop points of the runway

Every step stops and reports, without guessing, on PLAN-RUN §3's three,
and on these, which the plans inherit from MS3 part 3's and the roadmap:

1. **The roadmap's decision points**: R1 over 120 µs per live grob (MS3,
   at R1's, R2's and R3's closes); the B6 decision point (MS6, R7); s7 or
   a module boundary (any plan).
2. **Module boundaries**: the music crate naming a layout type or holding
   more than D108's slot; a crate beyond those the plan lists; the core
   growing beyond the calls behind 08 §4.2's ABI; `protocol.ts`,
   `driver.ts`, `host.ts` or the studio's tests changing (until R8, whose
   work that is).
3. **A function in no part**: a step that calls a function no plan
   ports, or one of `PRIMS`' placeholders no plan owns, stops (as step 35
   did for D131's six), and does not stand in for it.
4. **A capture that changes a reference** (`ref/<id>/…` not byte for byte)
   stops, unless the step's line is that capture.
5. **An output that changes where the step says none does** (traces,
   playback maps, `grobs.tsv`, `layout.tsv`, pages, stderr, diagnostics).
6. **Text widths**: a score failing C4 only where its text is placed asks
   whether part of W6 comes forward.
7. **Size of a change**: past a session's 2,500 lines at twice its top,
   make the part that fits and ask where the rest goes.
8. **The main checkout** before R6, and the studio's page (H2) before R8.
9. **The previous entry's Next** not matching LEDGER.md, or a file it
   names missing.

## 7. Numbering and places

- Steps keep their milestone's numbering: `MS3 · step 36` … `44`, then
  part 4's split is `MS3 · step 45`, and so on; MS4 starts at `MS4 ·
  step 0` (its plan). A letter suffix settles a stop point (`14a`).
- Decisions: D203 is the next free (D200); lanes reserve blocks.
- Session notes: `$LILY_CORPUS/device-runs/<date>-msN-stepK/`
  (`$LILY_CORPUS` is `~/.cache/lily-engine`); a split's scripts in
  `$LILY_CORPUS/msN-partK-split/`.
- `NEXT-SESSION.md` is kept only as a pointer to LEDGER.md (D200).

## 8. Measuring, as MS3 has done it

- **Full runs** at a3b67f4: 74 Rust result sets, 656 tests (3,893 s);
  184 tool tests (300 s); none skipped. Tests that need lilypond ran.
- **Sizes**: `node scripts/sizes.mjs` after `npm run build`; the driver
  row split into the core's functions and the rest; the code rows judged
  by origin (D225), by name beside.
- **Per-edit compile time**: step 35's `s8-time.sh` (the base built from
  `git archive` into the scratchpad, `cargo +stable build --release
  --example compile --target-dir <scratch>/basetarget`; 5 alternating
  runs; T02 T04 T20 T23 T30 T29 from `corpus.tsv` and `path.ly` with the
  emmentaler-20 and C059-Roman packs; the translations' ms and the whole
  compile's).
- **Devices**: step 23's `session5-drive.mjs` (desktop),
  `session6-run-emulator.sh` (emulator), NEXT-SESSION.md's *Set-up*
  recipes for the tablet (`svc power stayon true`, unlocked,
  Silk in front, `cpuset` `/top-app`, `stayon` false after) and the
  Simulator.
- **Lines (R12)**: `git diff <base>` over `crates/*/src/*`, non-blank
  added and removed, and added lines neither blank nor comment (`;`
  comments in Scheme preludes count as comments); the step's own port
  apart from wiring, borrowed lines, machinery and dump code.
- **Floats**: where a result could differ in its last bit, check the
  binary for fmadd/fmsub (otool/objdump) and port a fused one with
  `geometry::fma`.
