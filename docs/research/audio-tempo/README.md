# The score's tempo from its recording

How Lily Studio can find the tempo, bar lines and offset of a score's audio
track (D155), so that the score's MIDI, Karaoke and Export Video keep time
with the recording. The recording is the source of truth: the score's
playback is fitted to it, never the reverse. This note covers the tools
available in November 2026, what a spike on two test songs
showed, how a tempo that drifts could be kept with the score, and what to
build first. Nothing here is decided. A decision in `docs/DECISIONS.md`
should follow if the work goes ahead.

**[measured]** marks what was run on 2026-11-03 on the development Mac, with
`packages/transcribe`'s own environment (Beat This 1.1.0 `final0` on MPS,
librosa 0.11.0, torch 2.14) and GNU LilyPond 2.26.0. The recipe is in
Appendix A. Every other claim comes from the papers, READMEs and licence
files in Appendix B. A number from one paper should not be compared with a
number from another.

## 1. Where the studio is

- **D155.** A score names its recording with `% Audio: <file>`. The
  operator types `% Audio offset:` (seconds, to the hundredth) and `% BPM:`
  (quarter notes a minute, to the hundredth) into the Parts fold.
  `atTempo` (`playbackSetup.ts`) scales the parsed MIDI by the score's first
  tempo over `% BPM:`. This is one constant ratio: every note's time and
  length, the duration and midi.js's tempo map are scaled alike. The
  recording's place is the music's time less the offset.
- **D113.** lily-transcribe already finds a song's grid for *New score for
  …*. Beat This! finds beats and downbeats in all the stems mixed back
  together, and `grid.fit` (`packages/transcribe/src/lily_transcribe/grid.py`)
  turns them into a grid. It fits a straight grid through the beats and
  snaps it to a whole BPM when it is within 0.1 and fits as well. When at
  least 80 % of the beats lie within 40 ms of the grid, the song is
  *steady*. The downbeats vote for the meter and for bar 1. The result is
  cached per song in `~/.cache/lily-transcribe`.
- **D135.** For a drifting song, a transcribed score gets `songTempo`, a
  hidden `\tempo 64 = …` at every bar taken from lily-transcribe's
  `barTimes`. That puts the drift into the score's music, so the engine's
  MIDI follows the stem. A hand-written score with an audio track has
  nothing like it: D155's "Not done" says a drifting tempo is still D135's.

So most of what is needed already exists for stems. It does not yet exist
for a score's own recording, and it reaches the score only as notation.

## 2. Tools

### 2.1 Beat and downbeat trackers

| Tool | What it gives | Licence (code / weights) | Size, runtime | Accuracy (beat F1 / downbeat F1) | Fit for us |
| --- | --- | --- | --- | --- | --- |
| **Beat This!** (CPJKU, ISMIR 2024) | beats **and downbeats** at 50 fps, no tempo prior, no DBN needed | **MIT / MIT** | `final0` 78 MB (20 M params), `small0` 8 MB (2 M); torch | Ballroom 97.5 / 95.3, Hainsworth 91.9 / 80.0, RWC-Pop 96.1 / 93.7, SMC 62.7, GTZAN (test only) 89.1 / 78.3 | **already in lily-transcribe**; best measured, and the only MIT model that gives downbeats |
| `beat-this` crate (danigb) | Beat This in Rust, "minimal" peak picking | MIT | pure-Rust `rten` runtime or `ort`; bundles the small ONNX (~10 MB); the full one (~83 MB) is a download | as Beat This (port; parity not checked here) | the route to no Python at all, in the Tauri app |
| Beat This ONNX (community, `musetric/beat-this-onnx`) | `final0` for onnxruntime-web, WebGPU | MIT | 190 MB graph; WASM backend ~12× slower | parity claimed by the uploader, not checked | the only browser route with downbeats; heavy |
| madmom DBN trackers | beats, downbeats (DBN) | BSD / **CC BY-NC-SA 4.0** | no release since 2018; broken on Python ≥ 3.10 from PyPI | GTZAN 88.5 / 67.2 (TCN, Böck & Davies 2020) | **excluded**: the models are non-commercial (D113 already says so) |
| librosa `beat_track` | global tempo and beats (Ellis' DP), **no downbeats**; log-normal tempo prior at 120 BPM | ISC | already installed beside Basic Pitch | no standard F-measure found | a cheap cross-check, not a source of bar lines |
| Essentia `RhythmExtractor2013`, TempoCNN; essentia.js | BPM and beat ticks, no downbeats | **AGPL-3.0 / CC BY-NC-SA** | C++; essentia.js last released 2021 | TempoCNN Acc1/Acc2: GTZAN 69.4 / 92.6 | **excluded**: AGPL and non-commercial weights |
| BeatNet (Heydari 2021) | online/offline beats, downbeats, meter | CC BY 4.0 (code); weights unstated | torch + madmom, numba pinned to 0.54 | GTZAN offline 80.6 / 54.1 | worse than Beat This and needs madmom |
| aubio | tempo and beats, no downbeats | **GPL-3.0+** | C; last release 2019 | GTZAN online 57.1 | **excluded** for an MIT app |
| All-In-One (`allin1`) | BPM, beats, downbeats, **sections** (intro, verse, chorus) | MIT (weights tagged MIT, not confirmed) | torch, NATTEN, madmom from git, Demucs; last release 2023 | no extracted numbers | interesting for sections; too many dependencies and unmaintained |
| `stratum-dsp` (Rust), `web-audio-beat-detector` (JS) | BPM plus a grid, or BPM plus first-beat offset | MIT / Apache-2.0 | small, no model | own claims: ~88 % within ±2 BPM, ~12 % octave errors | constant BPM only, no downbeats; not good enough to set bar 1 |

Only Beat This! has MIT code **and** weights, gives downbeats, and runs in
each place the apps need: in Python today, in Rust through the `beat-this`
crate, and in a browser through ONNX.

### 2.2 Octave errors

Tempo papers count a tempo as right within ±4 % of the reference (Acc1),
and also at ×2, ×3, ×½ or ×⅓ (Acc2). The gap between Acc1 and Acc2 is the
octave-error rate. On GTZAN, madmom's tempo estimator goes from 69.7 to
95.0 and TempoCNN from 69.4 to 92.6: about a quarter of the songs are found
at the wrong metrical level. The usual cure is a prior: librosa's centres on
120 BPM, and madmom's DBN bounds the range to 55–215 BPM. Beat This has
neither, so it can count a ballad in eighths or a punk song in half-notes.

**We have a better prior than 120: the score itself.** It has a written
tempo (the first `\tempo`, or LilyPond's default ♩ = 60 when it has none) and
a time signature. Of the detected tempo's ×½, ×1, ×2 (and ×⅔, ×3⁄2 for a
compound meter), the level nearest the score's tempo is chosen. The score's
beats per bar replaces the downbeats' vote for the meter (§3.3). `% BPM:`
counts quarter notes, so in 6/8 a detected dotted-quarter beat is
multiplied by 1.5 before it is written.

### 2.3 Aligning the score to the recording

Beat tracking says where the recording's beats and bar lines are. It does
not say which of the recording's bars is the score's bar 1. That is a
question about the music, and the classic answer is to align the score's
synthesized MIDI with the audio by dynamic time warping (DTW) over chroma
features (Müller's synctoolbox, MIT code, with multiscale DTW and onset
features; librosa's `sequence.dtw`, ISC). Chroma is invariant to
instrumentation but not to transposition, so the 12 shifts are tried. §3.4
shows that on its own it is coarse on a riff-based song, and that it works
well once it runs on the beats Beat This found.

## 3. The spike

Two test songs, each with a score:

- **Song A** (3:37). The score `song-a-bass-tab.ly`
  has 102 bars of 4/4 at `\tempo 4 = 114`, with a slower 93 and 70 for its
  last three bars. Its `% Audio:` is the karaoke separation's instrumental,
  and `% Audio offset: -0.25` was set by hand.
- **Song B** (3:36). It has stems (bass, drums,
  guitar, other, piano, vocals), a karaoke separation, a transcribed
  `piano.ly`, and the score `song-b.ly`. That score is engraved from a
  printed sheet in G major: its tempo is only the words "Не спеша", it has
  its own form, and it was not written to this recording.

### 3.1 Tempo and steadiness [measured]

Beat This and `grid.fit` ran on each input, and librosa's `beat_track` ran on
the same audio. "F vs mix" is mir_eval's beat F-measure (±70 ms) against
Beat This on the full mix.

| Input | Beat This (s) | Beats on one straight grid | Grid ♩ | Bar 1 | F vs mix: beats / downbeats | librosa ♩ | librosa F vs Beat This |
| --- | --- | --- | --- | --- | --- | --- | --- |
| JMiD mix | 0.65* | 99.0 % | 113.76 | 0.325 s | — | 112.35 | 0.91 |
| JMiD instrumental | 0.65 | 99.0 % | 113.76 | 0.322 s | 0.996 / 0.945 | 112.35 | 0.83 |
| JMiD vocals | 0.65 | 57 % | fails | — | 0.07 / 0.01 | 112.35 | 0.01 |
| Song B mix | 0.67 | 95.1 % | 88.20 | 0.127 s | — | 89.10 | 0.93 |
| Song B instrumental | 0.64 | 95.1 % | 88.20 | 0.127 s | 1.00 / 1.00 | 89.10 | 0.89 |
| Song B stems mixed | 0.65 | 95.1 % | 88.20 | 0.127 s | 1.00 / 1.00 | 89.10 | 0.93 |
| Song B drums | 0.69 | 97.0 % | 88.21 | 0.137 s | 0.95 / 0.96 | 89.10 | 0.95 |
| Song B drums + bass | 0.62 | **100 %** | 88.21 | 0.134 s | 0.97 / 0.97 | 89.10 | 0.95 |
| Song B vocals | 0.63 | 40 % | fails | — | 0.17 / 0.10 | 117.45 | 0.19 |

\* With the model loaded. The first call took 1.68 s. A cold run of the whole
thing, from `python` starting through torch's import, the model's load,
decoding (0.36 s) and `grid.fit`, took **4.4 s** of wall time.

What it shows:

- **Both songs were recorded to a click.** Beats on the grid sit a median
  6–7 ms from it (95th percentile 16–20 ms), which is the 20 ms frame of
  Beat This. One `% BPM:` and an offset keep every beat of these songs
  within about 20 ms, and so does Song C in D113 (94 % within 40 ms
  at 123).
- **The tempo is not a whole number, and the hundredths matter.** At the
  score's 114, Song A's MIDI is **0.44 s** ahead of the record by
  its last beat. Song B's 88 against 88.20 is 0.47 s behind; the
  transcribed `piano.ly` already says "0.5 s off it by the end". `% BPM:
  113.76` keeps it within the 20 ms above. The rule is that drift = length ×
  ΔBPM ÷ BPM: 0.1 BPM at 114 over 210 s is 0.18 s. So lily-transcribe's snap
  to a whole BPM (`whole_tempo`, within 0.1) is right for a `\tempo`, which
  2.24 takes only whole, but `% BPM:` should get the fitted value.
- **The hand-set offset was 75 ms early.** Bar 1 of the record is at 0.325 s,
  so the offset that puts the score's bar 1 (MIDI 0) on it is **−0.33**. The
  operator's −0.25 is close, but 75 ms early is heard on a bass line.
- **The recording itself is enough.** The mix, the instrumental and the
  stems mixed back gave the same tempo to the hundredth and the same bar 1.
  A vocals stem alone does not work, nor does any stem without a beat.
- **librosa is no substitute.** Its global tempo is off by 1.4 and 0.9 BPM.
  A tempo bin is coarse, and over 3:30 that error is several seconds. It has
  no downbeats.
- **The raw downbeats need the grid.** In Song A, Beat This put a
  downbeat every two beats through the breakdown (148–164 s): 113 downbeats
  for 99 bars, 14 of them off the bar grid. In Song B's mix, the outro had
  doubled beats 80 ms apart. `grid.fit`'s vote and its tolerance ignore both.
  A bar map must be built the same way, from beats grouped by the meter, not
  from the raw downbeats.

### 3.2 A drifting take [measured]

Neither song drifts, and Song G (D133, D135), which does, is not on
this machine. So Song A's mix was made to drift: ten sections of
24 s, each sped or slowed by ffmpeg's `atempo` by 0, +3, −3, +5, −5, +2, −2,
+4, −4 and 0 %. That is an extreme live band, but its true bar lines are known
(the steady grid of §3.1, warped).

| | Bar lines, median error | worst |
| --- | --- | --- |
| The best single BPM and offset there could be (least squares through the true bars) | **239 ms** | 806 ms |
| A bar map: each true bar's nearest downbeat from Beat This on the warped audio | **11 ms** (95th pct. 43 ms) | past the last downbeat found (the outro) |

Beat This's beat F on the warped take was 0.985 and its downbeat F 0.905.
`grid.fit` correctly called it not steady (30 % on one grid). One number
cannot follow such a song. A map per bar follows it about as closely as it
follows a click track.

### 3.3 Stems for a cleaner beat [measured]

Drums + bass is the cleanest signal: 100 % of beats on the grid, every
downbeat on a bar line, and none of the outro's doubled beats. Drums alone
are almost as good. But the stems **start late**: Song B's drums come in
at 8.28 s, so the drums alone know nothing of the first three bars, and bar
1 must come from the mix. The tempo the two give differs by 0.01 BPM, which
is 25 ms over the song.

So stems are a refinement, not a requirement. When a separation of the same
recording is already on disk (`audio.siblings` finds it), its drums and bass
can give the beats where they play, with the mix for the rest. It is not
worth running a separation, which takes minutes, just to find the tempo.

### 3.4 Aligning by chroma [measured]

The scores were compiled to MIDI, and pretty_midi's chroma of the MIDI was
compared with librosa's `chroma_cqt` of the recording.

| Method | Song A | Song B |
| --- | --- | --- |
| DTW over chroma frames (21.5 fps), whole score against whole record, 5.2 s | bars 2–8 within ~50 ms, but only 40 % of the score's bars within 70 ms of a downbeat (90th pct. 0.53 s, worst 4.2 s). The repeated one-bar riff confuses it | fails: the subsequence DTW collapses onto 3 s of the record |
| DTW over **beat-synchronous** chroma (Beat This's beats; score beats from the MIDI), 2.1 s | **score beat 0 at 0.32 s, the detected bar 1; 99 % of the score's bars land exactly on a detected downbeat**; the path is 97 % diagonal; the score's slow last three bars fold onto the record's last beat | fails, as above |
| Bar 1 by correlation: the score's first 16 bars' chroma against the record's from each bar line, ~1 s | **recorded bar 0** (0.325 s), score 1.56 against 1.48 for the next candidate (bar 52, a chorus on the same chords) | bar 24 (65 s), not believable: the voice enters at ~21 s, in the score's bar 5 |

What it shows:

- On a score written to the record, **beat-synchronous DTW confirms the
  tempo and offset from beat tracking**. It also maps every score bar to a
  recorded bar, which is a bar map for free, and it shows where the two
  part. The score's ritardando is not in this recording, or Beat This found
  no beats in it.
- **Frame-level DTW is not precise enough** to set an offset on pop or
  rock, which repeats itself every bar or two. Beat tracking gives the
  phase, and DTW only needs to choose among bars.
- The transposition was found correctly both times (0; −2 for the G-major
  sheet against the F-major record). That is cheap insurance against a
  score in another key from the record.
- **No alignment can fix a score whose form differs from the record's.**
  Song B's sheet has a different form, a tone higher and no tempo: an
  intro of another length, repeats played otherwise. Structure-aware
  alignment exists (JumpDTW, Fremerey et al. 2010), but at that point the
  score should be written as the record plays, with its repeats unfolded.
  The tool should say that the score and the record do not match rather
  than guess.

## 4. One BPM or a map of the bars

**One `% BPM:` and an offset are enough for songs recorded to a click**,
which is most pop and rock since the 1990s and all three songs measured, as
long as the BPM keeps its hundredths. **A drifting song needs a bar map**:
a live take, an older record (Song G), a rubato ballad, a
ritardando or fermata. The steadiness test lily-transcribe already makes
(80 % of beats within 40 ms of one grid) chooses between them, and the best
line's residual says by how much.

### 4.1 Keeping the map with the score

In the spirit of D155's lines, so that the map goes with the score:

```lilypond
% Audio: song.mp3
% Audio offset: -0.33
% Audio bars: 2.108 4.216 6.33 8.441 10.55 12.659 14.771 16.88 18.99 21.1
% Audio bars: 23.207 25.32 27.43 …
```

- `% Audio bars:` holds the seconds from the score's bar 1 to each later
  bar line, in order, to the millisecond. Bar times are absolute, not bar
  lengths, so rounding does not add up, and a hand edit of one bar line
  moves only that bar. Several lines are read as one list, about ten numbers
  a line, so the editor stays usable. A 100-bar song is ten lines.
- `% Audio offset:` keeps its meaning: it places bar 1, so a nudge by hand
  still moves the whole recording. The map gives the shape.
- With a map, `% BPM:` is not used. The fold shows the map's mean tempo as
  its placeholder. A map shorter than the score goes on at its last bar's
  tempo, and a longer one is cut.
- Bars, not beats. Within a bar the warp is linear, and a live drummer's
  swing inside a bar is 10–20 ms. It matches D135's `songTempo` granularity
  and keeps the lines short. A beat map would be four times as long for
  little gain. A fermata inside a bar is the one case it misses.
- Rejected: a sidecar file (`<score>.beats.json`). It is one more file to
  carry and to lose, and D155 chose lines over the setup's `playback.json`
  because the tempo is the song's, not the window's (D149's reason). Also
  rejected: writing `\tempo` per bar into the music as D135 does. That
  changes the notation and recompiles, and 2.24's `\tempo` is whole numbers
  only.

### 4.2 Applying it without the engine (D144)

`atTempo` is the special case of one stretch for the whole piece. A bar map
generalizes it in the renderer (`atBars(midi, bars, barTicks)` in
`playbackSetup.ts`) and touches nothing under `packages/engine/` or
`timing.ly`:

1. Take the score's bar lines in ticks from the timeline the studio already
   has (the map's bars, as `barTime` reads them, converted to ticks through
   the MIDI's own tempo map).
2. Build a new tempo map with an entry at each bar's tick. Bar *k*'s tempo
   is chosen so that it lasts `bars[k+1] − bars[k]` seconds:
   µs a quarter = (seconds × 10⁶) ÷ (the bar's ticks ÷ division). Before bar
   1 (a pickup), use bar 1's tempo. After the map's last bar, use the last
   bar's tempo.
3. Re-time every note: seconds → ticks through the old map, ticks → seconds
   through the new one, for its start and its end. Do the same for the
   duration.
4. Replace `tempos`. `momentTime` then puts the engine's moments (Karaoke's
   syllables, the playhead, the staff of D156) on the warped notes, as it
   does after `atTempo`.

Everything that reads the scaled music today (the playhead, bars, the start
mark, the keyboard, Karaoke, Export Video, `withEnd`) reads the warped music
unchanged. A score with written tempo changes keeps them only where the map
does not override them. With a map, the recording decides.

## 5. Bar 1, pickups and intros

D155's sign: recording time = music time − offset. If the score's bar 1 sounds
at music time *S* and the recording's matching bar line is at *R*, then
**offset = *S* − *R***.

- *S* is 0 without a pickup. With `\partial`, *S* is the pickup's length at the
  played tempo (scaled by `% BPM:` or the map's first bar), so the pickup
  falls before *R* in the recording. A recorded pickup is itself not a bar
  Beat This marks, so nothing needs detecting for it.
- *R* is the recording's first bar line (`grid.fit`'s `bar_one`) when the score
  starts with the record. That held for both scores in §3.1. When the record
  has an intro the score leaves out, or the other way round, the bar-1
  correlation of §3.4 picks the bar. Its margin over the next candidate says
  how sure it is, and a weak margin is a question for the operator (§6).
  When the record's bar 1 is before the start, offset > 0 and the recording
  waits.
- Beat This marks a bar line only once the music has a beat. A free-time
  intro (a rubato piano before the band) has none, so its bar 1 is the
  band's. A score that writes that intro in bars still needs it placed by
  hand. Karaoke's `% Start:` is unaffected: it is where the view starts,
  not where the recording is.

## 6. Recommendation

**What to build first: *Detect from the recording*, filling `% BPM:` and
`% Audio offset:`.** It covers every click-track song, which is most of
what karaoke is sung to. It reuses Beat This and `grid.fit`, which are
already tested. It needs no new format, because the lines already exist.
The bar map is second, and it should wait for a drifting song in hand to
test it on (Song G).

**Where it runs.**

| | Mac app (Tauri) | Web (lily-studio-server) |
| --- | --- | --- |
| Now | a new `lily-transcribe --json tempo <audio> [--score-bpm N --beats-per-bar M]` subcommand. It runs the same `track_beats`, `grid.fit` (without the whole-BPM snap) and cache, and reports `{bpm, steady, beatsPerBar, barOne, bars, share, residual}`. crates/separate's `Transcriber` runs it as it runs `transcribe` (D113), and a Tauri command `detect_tempo` takes the score's `% Audio:` file, which `attach_stem` already lets into the scope. About 4–5 s for a song the first time, instant from the cache. | nothing: D155's web has no audio track. If it gets one, the server can run the same subcommand where it compiles. In the browser, Beat This in onnxruntime-web on WebGPU is possible but 190 MB for `final0` (the 8 MB `small0` scores 0.3 points of beat F lower on GTZAN, by the paper) |
| Later | the MIT `beat-this` crate (pure-Rust `rten`) in crates/separate, with `grid.fit` ported (~150 lines). The app then needs neither uv nor `packages/transcribe` to find a tempo; the small model is bundled or the full one downloaded once | the same crate built to WASM, if `rten` builds there (not checked) |

Licences: Beat This, torch, librosa and the `beat-this` crate are MIT, BSD
and ISC, which the apps can take. madmom's models, Essentia, TempoCNN and
aubio are not taken.

**The UI.** In the Parts fold, beside *Tempo, ♩ a minute* and *Starts at … s*,
a button *From the recording*, shown when the score has `% Audio:`. While it
runs it says *Listening…*, and the status line names the stage, as
*Transcribing…* does. Then:

- The fold writes `% BPM:` and `% Audio offset:` as **one undoable edit**,
  with D155's machinery (the cursor and view kept, default values taken
  out), and playback follows at once.
- The status line says what it found and how sure it is: *♩ = 113.76, 4/4,
  bar 1 at 0.33 s; 99 % of the beats on one tempo*. It also says when
  something is off:
  - the detected meter is not the score's;
  - the tempo was taken at another metrical level than detected
    (*heard ♩ = 227.5, taken as 113.76 to match the score's 114*);
  - bar 1 was matched past an intro (*bar 1 at the record's bar 5, 8.76 s*), or
    the match is weak (*the score's first bars do not match the record:
    check Starts at*);
  - the song drifts: *the tempo drifts ±3 %: one tempo keeps it within
    0.8 s*. Later, the same message offers *Follow the recording bar by
    bar*, which writes `% Audio bars:`.
- Nothing is written when the tempo cannot be found: too few beats, or a
  vocals-only file. The status line says why.

**How to verify it.**

- *Unit tests in packages/transcribe*, beside `test_grid.py`'s: a click
  track at a fractional tempo (113.76) with an offset, found to ±0.01 BPM and
  ±10 ms; the metrical level chosen by the score's tempo for a track at
  ×½ and ×2; a 6/8 track written as quarters; a pickup; an intro of four
  bars before the score's bar 1; a take warped as in §3.2, called not steady,
  its bar times within 50 ms; the CLI's events with Beat This replaced, as
  `test_cli.py` does.
- *Reference songs*, measured as in §3:
  - Song A: ♩ 113.76 ± 0.02, offset −0.33 ± 0.02.
  - Song B (with its transcribed `piano.ly`): ♩ 88.20 ± 0.02, bar 1 at
    0.13 s.
  - Song C (D113): ♩ 123.0x, bar 1 at 0.21 s.
  - Song G, for the map: every bar within 50 ms of D135's `barTimes`.

  The tolerance that matters is the error at the song's end. Every
  click-track song should end within 50 ms, which is the D113 test's own
  scale, well under D155's 0.25 s drift correction. That needs the BPM to
  0.02 on a 3–4 minute song, so the hundredths are just enough.
- *Renderer tests*: `files.test.ts` for the `% Audio bars:` lines (read
  across lines, written in place, taken out); `playbackSetup.test.ts` for
  `atBars`: a hand-made MIDI warped bar by bar, its tempo map, its moments
  on the same notes, equal to `atTempo` when the bars are even.
- *By ear in the app*: Song A with the detected values, the MIDI's
  bass against the instrumental at the start, a chorus and the end. D155 is
  itself "not heard in the app" yet, so this is the first time the two will
  be heard together.

## Appendix A: recipe of the spike

The scripts are in a scratch directory, not in the repository. Each runs in
`packages/transcribe`'s environment and imports its `audio` and `grid`:

```sh
P="uv run --project packages/transcribe python"
lilypond -dno-print-pages -o a song-a-bass-tab.ly     # a.midi
lilypond -dno-print-pages -o b song-b.ly                     # b.midi (the unfolded \score)
$P beats.py out/beats.json          # §3.1: Beat This (final0, MPS, no DBN) + grid.fit + librosa beat_track
                                    #       on each input; mir_eval F against the mix; saves beats/downbeats
$P grid_residual.py <beats> <downbeats> 114   # residuals to the straight grid; drift of a whole BPM
$P drift.py <mix> <beats> <downbeats> work/   # §3.2: atempo per 24 s section, true bars warped
$P dtw.py <score.midi> <audio> <downbeats>    # §3.4 frame chroma DTW (hop 1024 at 22.05 kHz, cosine)
$P dtw_beats.py <score.midi> <audio> <beats> <downbeats>   # beat-synchronous chroma DTW
$P barone.py <score.midi> <audio> <beats> <downbeats> 16   # bar 1 by 16-bar chroma correlation
```

Details that matter: transposition is the circular shift with the best
correlation of the whole song's chroma. Chroma is averaged per beat or per
bar with `librosa.util.sync`. `sync` makes no empty segment before a
boundary at 0, so the score's first segment must be kept when its first
beat is at 0. A first run that dropped it found everything one beat late.
pretty_midi's `get_beats`/`get_downbeats` follow the score's tempo and
time-signature changes.

## Appendix B: sources

- Beat This!: [paper](https://arxiv.org/abs/2407.21658), [code and licence](https://github.com/CPJKU/beat_this), [PyPI](https://pypi.org/project/beat-this/)
- `beat-this` Rust crate: [code](https://github.com/danigb/beat-this-rs), [README](https://docs.rs/crate/beat-this/latest/source/README.md)
- Beat This ONNX for the web: [musetric/beat-this-onnx](https://huggingface.co/musetric/beat-this-onnx) (unofficial)
- madmom: [code (BSD)](https://github.com/CPJKU/madmom), [models (CC BY-NC-SA 4.0)](https://github.com/CPJKU/madmom_models), [PyPI](https://pypi.org/project/madmom/)
- librosa: [PyPI](https://pypi.org/project/librosa/), [`beat_track`](https://librosa.org/doc/latest/generated/librosa.beat.beat_track.html), [`sequence.dtw`](https://librosa.org/doc/latest/generated/librosa.sequence.dtw.html)
- Essentia: [licensing](https://essentia.upf.edu/licensing_information.html), [models](https://essentia.upf.edu/models.html), [essentia.js](https://mtg.github.io/essentia.js/)
- TempoCNN and tempo accuracy (Acc1, Acc2): [Schreiber & Müller, ISMIR 2018](https://ismir2018.ismir.net/doc/pdfs/141_Paper.pdf); [tempo_eval on GTZAN](https://tempoeval.github.io/tempo_eval_report/gtzan.html)
- BeatNet: [paper](https://arxiv.org/abs/2108.03576), [code](https://github.com/mjhydri/BeatNet)
- aubio: [code (GPL-3.0)](https://github.com/aubio/aubio)
- All-In-One: [paper](https://arxiv.org/abs/2307.16425), [code](https://github.com/mir-aidj/all-in-one)
- Beat Transformer (comparison tables, TCN numbers): [paper](https://arxiv.org/abs/2209.07140)
- stratum-dsp: [code](https://github.com/HLLMR/stratum-dsp); web-audio-beat-detector: [code](https://github.com/chrisguttandin/web-audio-beat-detector)
- Alignment: [synctoolbox](https://github.com/meinardmueller/synctoolbox) (Müller et al., JOSS 2021), [libfmp](https://github.com/meinardmueller/libfmp); JumpDTW: Fremerey, Müller & Clausen, "Handling repeats and jumps in score-performance synchronization", ISMIR 2010
- In this repository: D113, D135, D144, D155 in `docs/DECISIONS.md`; `packages/transcribe/src/lily_transcribe/grid.py`; `packages/studio/src/renderer/playbackSetup.ts` (`atTempo`); `docs/research/audio-transcription/` §3.3
