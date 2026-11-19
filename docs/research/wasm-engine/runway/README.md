# The engine's runway: everything left, as a run of plans

This folder plans what is left of the engine (README §5, MS3 to MS8) as
an ordered set of **plans**, each run by Mother as a plan run of fresh
sessions, and coordinated through one ledger. It was written on
2026-10-18 at `feat/engine` a3b67f4, after MS3 step 35 (D200).

It adds nothing to the rule books. PLAN-RUN.md and IMPLEMENT.md
(committed only on `feat/studio-web` until R6: `git show
feat/studio-web:docs/research/wasm-engine/PLAN-RUN.md`) still govern
every step, and `progress.md` stays the log. The runway says **which plan
comes next, what it inherits, where it stops, and how plans hand over to
each other.**

| File | What it holds | Who updates it |
| --- | --- | --- |
| [`README.md`](README.md) | this page: the map, the order, the hierarchy, the decisions that shaped it | a plan that changes the order (with a decision) |
| [`PLANS.md`](PLANS.md) | each plan R1–R9: scope, needs, inherited open items, gate, budgets, stop points, estimate, exit | the plan's split step refines its own section |
| [`PROTOCOL.md`](PROTOCOL.md) | how a plan is started, split, run, closed and handed over; the step prompt's template; lanes; numbering | by decision only |
| [`LEDGER.md`](LEDGER.md) | where the runway stands: each plan's state, the current step, the last budgets, the next action | **every plan's closing step**, and any step that stops for the user |
| [`OPEN.md`](OPEN.md) | the register of open items, each owned by a plan | each closing step: closes, adds and re-assigns items |
| [`GRAND-PLAN.md`](GRAND-PLAN.md) | the task of one plan run that runs R2 to R9 in order, to R9's close | when the order or the stops ahead change |

## Start here

A new chat that runs the runway reads, in order: [`LEDGER.md`](LEDGER.md)
(where things stand, the next action), the plan's section in
[`PLANS.md`](PLANS.md), [`PROTOCOL.md`](PROTOCOL.md), the plan's items in
[`OPEN.md`](OPEN.md), then progress.md from the end. The prompt that starts
a plan is in PROTOCOL.md §2; the one that runs every plan left, R2 to R9,
in one run is [`GRAND-PLAN.md`](GRAND-PLAN.md).

## The map

```
            done                     the runway
MS0 ─ MS1 ─ MS2 ─ MS3 p1 ─ p2 ─ p3 (25–35) ┬ R1 ─ R2 ─ R3 ─ R5 ─ R6 ─ R7 ─ R8
                                           │  MS3 p3  p4   p5   MS4  merge MS6 MS7
                                           │  36–44        +close
                                           ├──────────── R4 (MS5) ──────┘ (before R7)
                                           └──────── R9 (MS8: W7 after R3; Y8… after R5)
```

**The default order is R1, R2, R3, R4, R5, R6, R7, R8, R9**, one plan at
a time in `.worktrees/engine`. R4 (MS5) and R9's W7 may instead run in
lanes beside the main one, in worktrees of their own, only when the user
opens them (PROTOCOL §5).

| Plan | Milestone part | In one line | Lines (latest estimate) | Needs |
| --- | --- | --- | --- | --- |
| **R1** | MS3 part 3, steps 36–44 | D88's translation cost; Y1's notation until the five lay out; C3/C4 on the five; R1 (µs per live grob) | 8,068 for 37–43 at 1.413, step 36 outside | step 35 (done) |
| **R2** | MS3 part 4 | Y2: skylines of every stencil, outside-staff, side positions, staff distances, empty staves; W5; W6's Emmentaler shaping; the cheaper first restore (D201) | ≈ 5,700 + W5 + W6 + the restore step | R1, and the user's answer if R1 stopped |
| **R3** | MS3 part 5, MS3's close | Y3: line and page breaking, broken items, spanner slices, titles; MS3's gate on the 10; MS3's summary | ≈ 4,500 | R2 |
| **R4** | MS5 | MIDI: performers, dynamics, controllers, marks, drums, `\unfoldRepeats`, `articulate.ly` | 6,500–9,500 | MS2 (done); lane B (PROTOCOL §5) |
| **R5** | MS4, in parts | Y4–Y6 (beams, slurs, ties, tuplets, lyrics, figured bass, tablature), the rest of W6, K4's split modules, S6's `fretted` and `chords`; C3–C5 on the 30 | 26,500–32,500 (× 1.415 likely) | R3 |
| **R6** | merge | `feat/studio-web`, then `feat/engine`, into `main`; D113–D119 renumbered (D202) | small; conflict work | R5 (and R4 if it ran on its own branch) |
| **R7** | MS6 | only what an edit changes: S5, re-parse, N8, Y7, W4, W8, H3; B6 on the phones | 15,500–24,500 | R5, R4, R6 |
| **R8** | MS7 | Lily Studio on the web: K5–K7, M7, H2/D58's `ScorePreview`, the server as fallback | 4,500–8,000 | R7 |
| **R9** | MS8 | beyond the corpus: W7 (PDF, lane C after R3), Y8, P12, the four iterators, S6's rest | 37,500–50,000 | R3 for W7; R5 for the rest |

Line figures are README §5.1's after MS1 (R12) and progress.md's
re-estimates at MS3 part 3's split; each plan's split re-estimates its
own at the previous part's rate.

## Hierarchy: runway, plan, step, session

- **Runway**: this folder. One.
- **Plan (R1…R9)**: one milestone part, or a whole small milestone. A plan
  has a **split step** (writes its step lines into progress.md, as MS3
  step 24 did), its **steps**, and a **closing step** (gate, devices,
  sizes, lines, LEDGER and OPEN). R1's split is done (part 3's lines at
  progress.md 17315–17335).
- **Step**: one numbered line of the plan (`MSn · step k`), the unit of a
  progress.md entry. Most steps need several sessions.
- **Session**: one fresh chat. A step that will not fit one session is
  run as a Mother plan of its own (as step 35 ran in nine), its sessions'
  notes in `$LILY_CORPUS/device-runs/<date>-msN-stepK/NOTES.md`.

So Mother runs **one plan run per runway plan**, started with the
driver prompt (PROTOCOL §2), whose steps are the plan's step lines, each
given a step prompt (PROTOCOL §3). A step bigger than a session runs as a
nested plan of sessions, as step 35 did.

## Decisions that shaped the runway (2026-10-18)

- **D200**: the engine's decisions take D200 and up (`main` holds
  D112–D123); the runway replaces `NEXT-SESSION.md` as the pointer.
- **D201**: the tablet's first restore (2.06 of Must) gets a step in R2
  (D88's lead: a cheaper first restore).
- **D202**: `feat/engine` joins `main` after MS4 (R6), before any work
  behind `?engine=1`; MS6 and MS7 run on a branch from `main`.
- Unchanged: D88 (a device miss over twice gets a step in the next part),
  R1's decision point (over 120 µs stops for the user, the emulator may
  decide when the tablet cannot be reached), budgets recorded and never
  gates, the straight port first with 05 §5's machinery allowed and
  recorded (answer 3 at part 3's split).

## What the runway does not decide

These stay the user's, and the plan that reaches one stops for it
(PROTOCOL §6): R1 over 120 µs (05 §5's machinery before MS4's notation);
s7 or a module boundary; the B6 decision point at MS6; a device miss over
twice whose lead the measurements do not name; the text family the core
writes (`serif` against D106's `LilyPond Serif`, OPEN B9); outlines in the
text packs and Apple's `sin` (OPEN E7, E8); the studio's page (H2, D58)
and anything in the main checkout before R6; the LilyPond 2.28 upgrade
(R10, after MS7).
