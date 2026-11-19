# The grand plan: R2 to R9 in one run

The task of one Mother plan run that finishes the engine: **every plan
the runway has left, R2 to R9, in order, to R9's close.** Written on
2026-10-22 at `feat/engine` 73fa32a, after R1's close, its stop's answer
and the devices' runs. It adds nothing to the rule books: PROTOCOL.md
says how each plan is split, run and closed, PLANS.md what each holds,
LEDGER.md where things stand. This page only strings them together.

## Read first

In `.worktrees/engine` (branch `feat/engine`):

1. [`LEDGER.md`](LEDGER.md): the next action. On 2026-10-22 it is **R2's
   split, MS3 step 45**.
2. [`README.md`](README.md), [`PROTOCOL.md`](PROTOCOL.md) (the cycle §1,
   the driver prompt §2, the step prompt's template §3, closing §4, lanes
   §5, stop points §6, numbering §7, measuring §8), [`PLANS.md`](PLANS.md),
   [`OPEN.md`](OPEN.md).
3. PLAN-RUN.md §2–§4 and IMPLEMENT.md's "Where to work", "Limits" and
   "Rules" (`git show feat/studio-web:docs/research/wasm-engine/…` until
   R6 brings them to `main`).
4. `docs/research/wasm-engine/progress.md` from the end: the last entry's
   Next and Open are the handoff. If its Next does not match LEDGER.md's
   next action, stop and report.

## The steps of this run

Propose these eight steps, one per plan, in this order (README's default
order, with R4 in the main lane between R3 and R5, PROTOCOL §5):

| Step | Plan | Milestone part | Done when |
| --- | --- | --- | --- |
| 1 | **R2** | MS3 part 4 | its closing step: C3 and C4 on Y2's eight (the five + T10, T13, T18), W5's check, R1 measured again |
| 2 | **R3** | MS3 part 5, MS3's close | MS3's gate: C3 and C4 on the 10, B10, W3's pixels; MS3's summary |
| 3 | **R4** | MS5 | C2 = 1.0 on the 30 and M01–M12, C2b on the 42 |
| 4 | **R5** | MS4, in parts | C3–C5 on the 30; each score with only its triggered modules |
| 5 | **R6** | merge into `main` | `feat/studio-web` then `feat/engine` merged, D113–D119 renumbered (D202); **asks the user first** |
| 6 | **R7** | MS6 | differential tests on E1–E3 and random edits; B6 on the phones |
| 7 | **R8** | MS7 | the engine ships in Lily Studio on the web, the server as fallback; every budget B1–B10 on the references |
| 8 | **R9** | MS8 | PDF (W7) and the long tail (Y8, P12, the iterators, S6's rest) to its gate |

**Each step runs one plan whole, as PROTOCOL §2's driver prompt runs it**,
and is far bigger than a session: it runs as a nested plan of sessions
(README's *Hierarchy*). Inside a step, in order:

1. **The plan's split** (PROTOCOL §1): the user's answers to the plan's
   questions first (PLANS.md's *Questions its split asks the user*), the
   counts with kept scripts, the step lines in progress.md, PLANS.md's
   section and LEDGER.md. Commit.
2. **The plan's steps**, one per step line, each with a step prompt from
   PROTOCOL §3's template; a step over a session runs in sessions that
   finish whole parts, commit and hand off (`sessionK-handoff.md`), its
   last session running the full suite and writing the progress.md entry.
3. **The plan's closing step** (PROTOCOL §4): its gate with nothing
   skipped, the earlier gates, the devices, sizes, lines against the
   estimate, D88's and R2's reports, then OPEN.md and LEDGER.md. Commit
   and push.

A step of this run ends when LEDGER.md names the next plan's split as
the next action. The next step starts by reading LEDGER.md and
progress.md's last entry again, never from this page's memory.

## Stops: ask, then wait

Every stop point of PROTOCOL §6 and PLAN-RUN §3 stops the run for the
user; it does not skip ahead, since each plan needs the one before it.
Put the question with the options and a recommendation, record the
answer where the protocol says (LEDGER.md's Next action and *History*,
the split's *Settled with the user*), and go on. Known ones ahead:

- **R2's split asks E8** (Apple's `sin` bit for bit for circle and
  ellipse skylines). B9 (`LilyPond Serif`) and E7 (outlines in the text
  packs) are answered: LEDGER.md.
- **R1 at R2's machinery step, at R2's close and at R3's close**: still
  over 120 µs on the tablet stops for the user. On 2026-10-22 the tablet
  measured 810.4 µs a live grob and the floor of 05 §5.1's machinery
  projects to ≈ 190–220 µs there, so expect **the budget's question
  (option 3)** with that step's figure.
- **D88**: a device miss over twice gets a step in the next part; one
  whose lead the measurements do not name stops.
- **Module boundaries, a function in no part, a capture that changes a
  reference, an output that changes where none should, text widths, a
  change past 2,500 lines at twice its top** (PROTOCOL §6).
- **R6 asks before touching `main`**; nothing in the main checkout
  before it.
- **R7's B6 decision point** (MS6).
- **The studio's page (H2, D58) before R8.**
- **Lanes:** R4 runs in the main lane unless the user opens
  `.worktrees/engine-midi`; W7 may run in lane C (`.worktrees/engine-pdf`
  on `feat/engine-pdf`) after R3 only if the user opens it. Ask once,
  at R3's close, whether to open lane C.

## Devices

Every closing step measures on the devices (PROTOCOL §8). **Ask the
user to attach the Android tablet and the iPhone** before a
plan's close; use the emulator in the tablet's place only when the user
says the tablet cannot be attached, and say so in the entry. The
2026-10-22 scripts are the starting point, in
`$LILY_CORPUS/device-runs/2026-10-22-r1-devices/` (`$LILY_CORPUS` is
`~/.cache/lily-engine`): `run-tablet.sh` (Silk over CDP, brought to the
front with `monkey`), `run-iphone.sh` with `iphone-open.mjs` (Safari by
`devicectl` over HTTPS with mkcert's root), `run-android-phone.sh`,
`run-notemax.sh` with `android-open.mjs` (a browser Playwright cannot
drive over CDP, pages opened by intents), and `sum.mjs`. A run copies
them into its own dated folder. Each device's state is left as found:
`stayon false`, tunnels removed, servers stopped. The A15-class Android
(the floor) has never run (OPEN D4): ask for it at R8 at the latest,
whose gate needs it.

## Rules for the whole run

- **Where:** only `.worktrees/engine` on `feat/engine` until LEDGER.md
  says R6 has merged; then the branch LEDGER.md names, from `main`. Never
  the main checkout before R6. New worktrees only under `.worktrees/`
  (never a sibling directory), and only for a lane the user opened.
- **Commits:** each session ends with a commit; each step's last
  session and each closing step pushes `feat/engine`. Author as the
  repository's git identity; end each message with the attribution the
  session is given.
- **Decisions** take the next free number at D200 and up (D205 on
  2026-10-22); a lane's split reserves a block in LEDGER.md.
- **Tests:** a run with skipped tests has not verified a change to
  compiling or parsing (AGENTS.md). Each step's last session runs the
  full suite (`npm run test:rust` with `LANG=en_US.UTF-8`, `npm run
  test:tools`, `npm run lint:rust`, the root's `npm run lint`); host
  faults are rerun, not counted as passes.
- **The rule books:** AGENTS.md's rules (nothing under
  `packages/common` imports `vscode`; the studio's Rust compiler and
  error parser change with the extension's; `completions.json` is
  generated), IMPLEMENT.md's limits, and DECISIONS.md for anything that
  changes a convention.
- **Leftovers:** the worktrees `.worktrees/grand-plan-runway-run-the-engine-s-next-0[1-5]`
  (at 4d2f50b, clean) are from R1's grand plan; do not use or remove
  them without asking.

## Done

The run is done when **R9's closing step** has passed MS8's gate and
LEDGER.md says so; MS7 (R8) is where the engine first ships. If the run
must end earlier (a stop the user leaves open, a session limit), the last
commit, LEDGER.md's Next action and progress.md's last entry are the
handoff, and a new run started with the same prompt picks up from them.
