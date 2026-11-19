# The sung line's own timing

How Lily Studio could find, from a score's recording, when each note of the
vocal line is actually sung, so that the melody's playback and Karaoke's
words follow the singer instead of the written rhythm. The score is written
in quarters, eighths and sixteenths, and a singer never sings exactly on
them. This note covers the tools and models available in November 2026,
what a spike on a test song showed, what "when a note is
sung" means, how the sung times could be kept with the score, and what to
build first. Nothing here is decided. A decision in `docs/DECISIONS.md`
should follow if the work goes ahead.

**[measured]** marks what was run on 2026-11-06 on the development Mac, in
`packages/transcribe`'s environment (torch 2.14.1, torchaudio 2.11.0,
librosa 0.11.0), with karaoke-mlx's `becruily` model for the lead vocals and
GNU LilyPond 2.26.0. The recipe is in Appendix A, and the scripts are in
`spike/`. Every other claim comes from the papers, READMEs, model cards and
licence files in Appendix B. A number from one paper should not be compared
with a number from another.

**Licences.** Lily is for personal, non-commercial use (D169). Weights under
CC BY-NC or BY-NC-SA can be used if they are downloaded on first use and named
in their package's README. Weights with no licence, or for "research use
only", cannot. Copying non-commercial code into the source is not allowed
either.

## 1. Where the studio is

- **D155, D159.** A score's recording (`% Audio:`) is placed by
  `% Audio offset:` and stretched by `% BPM:`, which *From the recording*
  finds with Beat This. That is one straight line through the whole song.
- **audio-tempo §4.** A bar map (`% Audio bars:`), the seconds of each bar
  line, is designed for songs that drift but is not built. Within a bar, the
  warp is linear.
- **D133, D146.** Karaoke shows a syllable when its note's moment comes,
  through `momentTime` on the parsed MIDI. Its times are therefore the
  score's grid, moved only by the tempo.
- **D134.** *New score for vocals* hears the words with Whisper's word times
  and lays them on transcribed notes. That is the reverse case: the notes
  come from the audio. Here the notes and words are known, and only their
  times are wanted.

So today every sung note is at its written place on the beat grid. This note
is about the step below the bar: the time of each note.

## 2. What the problem is

This is **score-informed alignment**. It is easier than transcription,
because the pitches, the order of the notes and the syllables are known, and
the beat grid already says where each note should be to within a beat. Three
sources of evidence are in the vocals stem:

| Evidence | What it marks | Strong when | Weak when |
| --- | --- | --- | --- |
| **The words** (phonetic forced alignment) | where each syllable's consonant and vowel start | every syllable, including repeated notes on one pitch | melismas (one syllable, many notes); breathy or growled singing; a stem with other voices in it |
| **The pitch** (an f0 track) | where the voice moves from one pitch to the next | a legato line with leaps; melismas | repeated notes on one pitch; glides and scoops; a score whose pitches are wrong |
| **Onsets** (spectral flux of the stem) | where a new sound starts | detached singing, plosives | legato, voiced consonants (m, n, l) |

None of them is enough alone. A syllable has no pitch change when it repeats
the note before. A melisma has no consonant. So the published systems that do
this well combine at least two, and none of them is released in a form we
can ship (§3).

### 2.1 When is a note "sung"?

A sung syllable has no single instant. The consonant starts before the beat,
the pitch moves during it, and the vowel lands on or after the beat. On the
spike's song **[measured]**, the consonant started a median 58 ms before the
grid, the pitch crossed to the new note 48 ms before it, and the vowel started
51–64 ms after it. That is 110–120 ms from the first sound to the vowel. So
the time depends on what it is for:

- **Karaoke's word** should light at the **consonant**. That is when the
  singer is heard to start the word.
- **The melody's MIDI note** should start at the **pitch arrival** or vowel.
  A note started at the consonant sounds early against the voice.
- **A bar line or beat** is neither. Singers lay the vowel near the beat,
  and the beat map stays the drums' (D159).

A tool that gives only "the onset" hides that difference. The format of §5
keeps the syllable's and the note's times apart.

## 3. Tools

### 3.1 Aligning known words to singing

What the field reports on JamendoLyrics (pop and rock mixes, 79 songs) is
**AAE**, the mean absolute error of word onsets, and **PCO**, the share of
words within 0.3 s.

| System | Approach | Code / weights licence | Languages | Accuracy on singing | Fit for us |
| --- | --- | --- | --- | --- | --- |
| **torchaudio `forced_align` + `MMS_FA`** | CTC Viterbi over a wav2vec2 model's frames, uroman text, a `<star>` token for audio the text does not cover | BSD-2 / **CC BY-NC 4.0** | 1,130, Russian through uroman | none published; best of the spike (§4) | **the first choice**: the best measured, usable under D169 when downloaded on first use. `forced_align` was deprecated in torchaudio 2.8 and kept in 2.10 |
| torchaudio `WAV2VEC2_ASR_BASE_960H` | the same, an English speech model | BSD-2 / **MIT** | English | none published; the spike's vowels a median 168 ms late (§4) | permissive but worse on singing |
| jonatasgrosman XLSR-53 models (as WhisperX uses) | the same, per language | **Apache-2.0** | ru (13.3 % WER, Common Voice), en and others | none published | not measured. The permissive route to Russian in CTC |
| ctc-forced-aligner (MahmoudAshraf97) | the same with a memory-light C++ Viterbi | BSD-2 / default model **CC BY-NC 4.0**; any HF CTC model can be swapped in | as the model | none published | a better Viterbi over the same MMS weights |
| **NeMo Forced Aligner** | Viterbi over FastConformer CTC / hybrid models | Apache-2.0 / **CC BY 4.0** (attribution) | en, ru (`stt_ru_fastconformer_hybrid_large_pc`) | none published; "only English tested" | permissive weights for en and ru; a heavy install on a Mac |
| **Montreal Forced Aligner** | Kaldi GMM-HMM, phone and word boundaries | MIT / **CC BY 4.0** (en, ru); Russian CV model CC0 | en, ru, many | on Chinese singing, 78.5 ms mean character error (DeepSinger) | **expects line-length segments**, which the score's phrases give (§4). Phone boundaries, so consonant and vowel apart. The strongest permissive candidate |
| **SOFA** (singing-oriented forced aligner) | a neural aligner built for singing, from DiffSinger's dataset work; breath and silence detection | MIT / per model: English `tgm_en_v100` "commercial use approved" in a forum post, no licence file; Russian `hhskt_ru` no licence, attribution asked | zh mainly; community en, ru, ja | BER 20.9, IoU 80.0 at 20 ms tolerance vs MFA's BER 40.3 (STARS' table, Mandarin) | built for exactly this, but the weights' terms are a forum post. Expects 5–15 s slices |
| HubertFA | SOFA's successor, HuBERT encoder, ONNX inference | Apache-2.0 / no licence on the releases | zh, ja (en dictionary bundled, quality unknown) | none published | no Russian, no licence |
| Huang, Benetos & Ewert 2022 (joint pitch detection) | phoneme × pitch posteriorgram on separated vocals, Viterbi | MIT / checkpoints in the repo, no licence stated, trained on DALI | English | Jamendo AAE 0.23 s, PCO 94 % | the idea we want (words and pitch together), English only, weights' terms unknown |
| Durand, Stoller & Ewert 2023 (Spotify) | contrastive audio–text matching | **not released** | en, es, de, fr | Jamendo AAE 0.15 s, PCO 92 % | the state of the art, unavailable |
| Cheng, Nakano & Goto, DAFx 2025 | masked frame-wise phoneme CE | no code found | — | mean 0.216 s, **median 0.041 s** | shows the mean is driven by outliers |
| AutoLyrixAlign | Kaldi | **GPL-3.0** | English | Jamendo AAE 0.22 s | excluded |
| Whisper word times (D134), stable-ts `align()` | cross-attention timing | MIT (stable-ts archived 2026-05) | many | no benchmark; cross-attention timing is known to be loose | what the studio has; a fallback |

Two lessons from the literature:

- **The best open systems reach a mean of 0.15–0.2 s on whole songs, but a
  median of about 40 ms.** The errors are a few large failures (a repeated
  "la la", a line the singer skips), not a uniform blur. A score that bounds
  each phrase's window removes most of them (§4).
- Kick et al. (2024) found that their model, trained on full mixes, got
  *worse* on separated vocals. Speech models such as wav2vec2 go the other
  way: they want the stem.

### 3.2 Aligning a score's notes to audio

| System | What it does | Licence | Accuracy | Fit for us |
| --- | --- | --- | --- | --- |
| **parangonar `AudioToScoreMatcher`** (Peter, Hu & Widmer, 2026) | 88 band-pass filters, onset and spectral features, a DP matcher over the score's onset groups with a tracked beat period; no synthesis, no transcription | **Apache-2.0**, no weights; v3.4.1, 2026-10-27 | (n)ASAP piano: mean 86 ms, **median 21 ms, 83.7 % within 50 ms**; synctoolbox on the same data 135 / 49 ms, 53.2 % | the best released score-to-audio aligner. Tested on piano only; onsets only. Its filterbank range is a parameter, so a vocal range could be set (untested) |
| parangonar symbolic matchers (`DualDTWNoteMatcher`, `SubPartMatcher`) | match a performance's notes (seconds) to a score's notes (beats) | Apache-2.0 | 98.1 % within 50 ms, given a good transcription | the second half of "transcribe, then match". Needs partitura note arrays; partitura does not read LilyPond, but our MIDI suffices |
| **SOLEA's alignment** (Pérez Fernández et al., SMC 2025) | snaps detected onsets to score onsets within τ = 75 ms of a reference alignment, by monotonic bipartite matching; reliability by segment | **no licence file**; about 60 lines of NumPy | indirect only: a transcriber trained on its re-aligned DALI scores COn 81.1 vs 74.8 on MIR-ST500 | the idea transfers: our beat grid is the reference alignment, and the snap is small. Reimplement, do not copy |
| synctoolbox (Müller et al.) | MrMsDTW over chroma and DLNCO onset features | MIT; v1.4.2, 2026-05 | none in its own paper; 135 ms mean on piano (above) | audio-tempo §3.4 used it for bars; too coarse for notes |
| Murgul et al. 2025 | a CRNN that refines a DTW's piano roll against a CQT | not released | piano: 12 ms mean, 95.6 % within 50 ms | piano only, unavailable |
| pyAMPACT (Devaney et al., ICMC 2025) | score–audio DTW, then per-note onset, offset, f0, tuning and dynamics, to MEI | MIT on PyPI, no licence file in the repo | none in ms | the only one that also gives **offsets**, built for singing research. Worth a test |
| Park et al. 2024 (KAIST) | online warping on chroma and phonetic posteriors, classical singing | not released | Winterreise: median 136 ms, 64.8 % within 200 ms | real-time; shows how hard sung classical voice is |

### 3.3 Singing transcription tools from singing-voice synthesis

The DiffSinger and OpenVPI community build their training data from audio and
lyrics: forced alignment gives phoneme durations, then a note extractor splits
each word into notes in seconds. That is close to our problem, but **none of
their tools takes a known score**.

| Tool | What it does | Code / weights | Note |
| --- | --- | --- | --- |
| **GAME** (OpenVPI, 2026) | note boundaries by discrete diffusion, then pitch. `align` takes **fixed word boundaries** and fills in the notes between them | MIT / **CC BY-NC-SA** | the closest to "known words, find the notes", and usable under D169: MMS_FA's syllables as its fixed word boundaries, GAME for the melismas between them. ±50 ms quantity error 0.123 on its own held-out singers |
| SOME (OpenVPI) | audio to MIDI notes per word | MIT / **NC** | superseded by GAME |
| ROSVOT (ACL 2024), STARS (ACL Findings 2025) | note transcription aligned to word boundaries; STARS also aligns phonemes and labels techniques | MIT / no licence, trained on NC data (M4Singer, GTSinger) | Mandarin (STARS also English). COn F1 94.0 (ROSVOT) |
| MakeDiffSinger | the pipeline: slice 5–15 s, `.lab` syllables, MFA or SOFA, then RMVPE or GAME | BSD-3 | for "polysyllabic" languages (English, Russian) its README says to group phonemes into words **by hand** |
| VocalParse (2026) | a Qwen3-ASR fine-tune that writes lyrics with pitch and note values | Apache-2.0 / Apache-2.0 | note values, no seconds, Mandarin. Not this task |

### 3.4 Pitch trackers for the stem

| Tracker | Code / weights | pitch-benchmark F1 | Speed (× real time, one core) | Note |
| --- | --- | --- | --- | --- |
| **SwiftF0** | MIT / in the package (trained on research-only data, a legal question) | **0.781** | 180 | numpy + onnxruntime, 14 k parameters, a `segment_notes` built in |
| RMVPE | Apache-2.0 / the common weights have no licence file; RVC's copy "research use only" | 0.768 | 13.6 | the de-facto choice on vocal stems, used by OpenVPI, ROSVOT and STARS |
| FCPE (torchfcpe) | MIT / the bundled model unlicensed; the paper's model NC | 0.728 | 27.8 | |
| torchcrepe | MIT / MIT | 0.691 | 0.4 | slow |
| PESTO | LGPL-3.0 / in the package | 0.680 | 20 | fine as a sidecar |
| pYIN (librosa) | ISC, no model | not in that benchmark | 3.7 **[measured]** (58 s for 3:37) | what the spike used, already installed |

## 4. The spike [measured]

**The song.** The test song is audio-tempo's Song A (3:37),
`song-a-bass-tab.ly`, with its D159 values (`% BPM: 113.76`,
`% Audio offset: -0.33`). Its lead vocal has 239 notes and 187 syllables,
written as a person transcribed it. The song is recorded to a click (99 % of
beats on one tempo, audio-tempo §3.1), so the bar grid itself is right, and
everything below is the voice against it. The lead vocals were separated from
the record with karaoke-mlx (`becruily`).

There is **no ground truth**. Nobody has marked where each syllable is sung,
so the methods are compared with the grid and with each other. Agreement
between independent methods is evidence. It is not accuracy.

| What was measured | Result |
| --- | --- |
| **MMS_FA over the whole song in one pass** | 27 % of syllables more than 0.3 s from their grid time. The aligner spent the intro's words on the first verse and lost its place 15–20 s near the end |
| **MMS_FA phrase by phrase**, each phrase (the score's full stops) in a window from its first note − 0.6 s to its last note's end + 0.6 s, `<star>` between syllables; 19 s on CPU | 14 % more than 0.3 s off. **18 of those 29 are the intro's three sung lines, which the lead stem does not contain** (0 % of 0–30 s is voiced; the record has it in the backing). The aligner's own score marks them, 0.00–0.01 |
| the same, the least confident quarter dropped | **4 %** more than 0.3 s off. The vowel a median **+51 ms after the grid, IQR +15 … +80 ms**. Section by section, medians +41 to +73 ms |
| consonant onsets (MMS, phrase-windowed) | a median 58 ms before the grid, IQR −97 … +31 ms |
| `WAV2VEC2_ASR_BASE_960H` (MIT) instead, phrase-windowed | vowels a median **168 ms** after the grid, 31 % more than 0.3 s off. The English speech model lags on sung vowels |
| **pitch crossings**: for the 79 legato steps of 2 semitones or more, where the f0 (pYIN) crosses to the new note's side for 50 ms | a median 48 ms **before** the grid, IQR −192 … −20 ms. Only 37 % within 50 ms of the grid |
| crossings against MMS vowels on the same 68 syllables | the crossing a median 98 ms before the vowel, IQR −251 … −44 ms: the pitch moves on the consonant (§2.1), but the spread is wide |
| the written pitches against the sung f0 (±75 cents, 200 notes with enough voiced frames) | **153 of 200 (77 %)** as written. The rest are written wrong, ornamented, or the f0 is wrong |
| a Viterbi over each note's shift (pitch, onset strength, a pull to the grid, a smoothness cost) | it stayed on the grid (median 0, IQR −20 … +10 ms): the pull and the smoothness were too strong. No result, but it shows the danger: **a prior strong enough to stop failures also hides real timing** |

What it shows:

- **The score bounds the search, and that is most of the gain.** Phrase
  windows halved the failures. The aligner's confidence then found nearly all
  that remained, including a whole section the stem does not sing. A tool
  that does not know the score has neither.
- **On a click-track song, the written rhythm placed on the beat grid is
  already close.** Most syllables were within about 100 ms of their grid time.
  The singer's own deviations were tens of milliseconds, consistently a little
  behind the beat on the vowel. For Karaoke, that is the size of the gain.
  The larger gains should be on rubato, ballads, live takes and older records,
  and the spike did not have one of those (§7).
- **The measuring tools disagree at the scale of the effect.** Two independent
  measures (phonetic, pitch) differ by 50–100 ms on typical syllables. Until
  some syllables are marked by hand, no method here can be said to be within
  50 ms.
- **The non-commercial model was the best.** MMS_FA worked; the MIT English
  model tried was clearly worse. Lily is for personal use (D169), so MMS_FA
  can be used when downloaded on first use. The permissive candidates (MFA,
  NeMo, the Apache XLSR models, SOFA's English model) stay untested here and
  matter only if Lily is ever sold.
- **The score is not always right.** 23 % of notes did not match the singer's
  pitch. An aligner that trusts the score's pitches must tolerate that. That
  same check is a by-product worth showing (§5.3).

Two listening files were made from 30–120 s: the lead vocals on the left, a
short tone at each note's pitch on the right. One has the notes at the grid,
the other at the MMS consonant onsets where the aligner was confident. They
are in the spike's scratch directory, not the repository.

## 5. How it could fit the studio

### 5.1 The pipeline

```
recording ──karaoke-mlx──▶ lead vocals stem                       (D112, exists)
score ──lilypond──▶ the voice's notes and syllables, on the bar map (D159, audio-tempo §4)
stem + syllables, phrase by phrase ──forced alignment──▶ consonant, vowel, confidence per syllable
stem ──f0──▶ pitch track ──▶ pitch arrivals inside melismas and on leaps
both ──snap to the grid within ±τ, confidence-gated──▶ each note's onset and offset, or "keep the grid"
```

- **Windows from the score.** Each phrase is aligned only within its own
  grid window, with a wildcard token for breaths and other voices.
- **Snap, don't search.** As in SOLEA, each note may move only within τ of
  its grid time (τ about a 16th, 130 ms at ♩ = 114). It moves only when the
  evidence agrees. Otherwise it keeps the grid. A missing or low-confidence
  syllable falls back to the grid, never to a guess.
- **Melismas** take their notes' times from the pitch track, between the
  syllable's vowel and the next syllable's consonant, in order.
- **Offsets.** A note ends where the next one starts, or where the voice
  stops for 80 ms before a rest. Only pyAMPACT, of the released tools,
  estimates sung offsets.

### 5.2 Keeping it with the score

The sung times belong to one voice, not to the song. They follow the spirit
of D155's lines and audio-tempo §4.1:

```lilypond
% Audio: song.mp3
% Audio offset: -0.33
% BPM: 113.76
% Audio sung lead: 239 9f3a
% Audio sung lead: +12 -40 . . +31 +8 …
```

- `lead` names the `Voice` (D138 already reads voice names for Karaoke's
  lanes). The first line gives the voice's note count and a short hash of its
  pitches and durations. When the operator edits the line, the stored times no
  longer belong to it. The studio then ignores them, and its status line says
  so, rather than putting old times on new notes.
- Each note's shift is in milliseconds from its grid time on the bar map,
  with `.` for "the grid". Shifts are short and keep the map's meaning: with
  no shifts, the line plays as written.
- A syllable's consonant lead (Karaoke) and the note's arrival (MIDI) differ
  by about 100 ms (§2.1). The simplest form stores the note's time and lets
  Karaoke light each syllable a fixed 60–80 ms earlier. If that proves too
  crude by ear, a second line per voice can carry the syllables.
- Rejected: writing the sung rhythm into the notation, as 32nds or tuplets.
  It makes the score unreadable, and no rhythm is sung exactly anyway.
  Rejected: a sidecar file, for D155's reason.

### 5.3 Applying it (D144)

This is a renderer step after `atTempo` or `atBars`, and it touches nothing
under `packages/engine/` or `timing.ly`:

1. The voice's notes in the parsed MIDI (its track, as D138 finds it) are
   matched to the stored shifts in order.
2. Each note's start moves by its shift. Its end moves to its stored offset,
   or to the next note's start when legato.
3. **A time map for the voice**: a piecewise-linear map from the grid's
   seconds to the sung seconds, through every note's start. `momentTime` uses
   it for the voice's own moments, so Karaoke's syllables, D156's staff and
   the playhead on that line follow the singer. Every other part keeps the bar
   map.

The comparison also gives the operator something new: **"the score differs
from the singer here"**. That includes notes moved by more than τ, pitches not
sung as written (23 % in the spike), and phrases the stem does not sing (the
intro). Shown as marks on the score, it is a proofreading tool for a
transcription by ear, and it needs no sung times stored at all.

## 6. Recommendation

1. **Hand-mark a reference first.** Two or three phrases of Song A,
   and the same of a slow song, marked to the consonant and the vowel by ear
   (for example in Audacity's label track). Nothing in §4 can be called
   accurate without it, and it is an hour's work.
2. **Then measure against it** the phrase-windowed MMS_FA that
   `spike/align_lyrics_windowed.py` already has, which is the candidate under
   D169 (downloaded on first use, 1.18 GB). For the notes inside a syllable,
   compare GAME's `align` (its word boundaries fixed from MMS_FA) with
   SwiftF0's or RMVPE's pitch crossings, and try parangonar's
   `AudioToScoreMatcher` with a vocal filterbank. MFA is the one other
   aligner worth a run: it gives phone boundaries (consonant and vowel apart)
   and has a Russian model. The bar is 50 ms on most syllables.
3. **Build "the score differs from the singer here" before storing any
   times.** It needs only the comparison, it is useful even when the
   alignment is rough, and it surfaces where the alignment fails.
4. **Then `% Audio sung <voice>:`** and the renderer step of §5.3, after
   audio-tempo's bar map, which it sits on.

Where it runs: like D159, as a `lily-transcribe --json sung <recording>
<score.midi> --voice lead` subcommand in the Mac app, using the stem the
studio already separates. Nothing on the web until the web has audio tracks.

## 7. Open questions

- The spike had one click-track rock song. A rubato ballad with its
  recording is needed to see whether the gain is larger where the grid is
  worse, as expected.
- Russian: MMS_FA reaches it through uroman's romanisation, which may blur
  sung Russian vowels. MFA's Russian model is the comparison. Neither is
  measured.
- How much earlier Karaoke should light a syllable than its note (§5.2) is a
  matter for the ear, not a number from a paper.
- A voice the karaoke model leaves in the backing (the intro here) cannot be
  aligned from the lead stem. Whether to align against the full mix or the
  backing there is untested.

## Appendix A: recipe of the spike

The scripts are in `spike/`. Each runs in `packages/transcribe`'s environment
(`P=packages/transcribe/.venv/bin/python`; MMS_FA downloads 1.18 GB on first
use, the English model 360 MB):

```sh
lilypond -dno-print-pages -o score song-a-bass-tab.ly                 # score.midi
(cd packages/separate && uv run karaoke-mlx separate song.mp3 -o out/) # "song - vocals.wav"
$P -I spike/prepare.py score.midi notes.json --bpm 113.76 --offset -0.33   # lead track 1, lyrics track 7
$P -I spike/features.py vocals.wav feat.npz                  # pYIN f0, onset strength, 10 ms
$P -I spike/align_pitch.py notes.json feat.npz pitch.json    # the Viterbi of §4 (stuck on the grid)
$P -I spike/align_lyrics.py notes.json vocals.wav lyrics.json            # MMS_FA, whole song
$P -I spike/align_lyrics_windowed.py notes.json vocals.wav lyr_mms.json mms   # phrase windows
$P -I spike/align_lyrics_windowed.py notes.json vocals.wav lyr_en.json en     # WAV2VEC2_ASR_BASE_960H
$P -I spike/compare.py pitch.json lyr_mms.json               # the tables of §4
$P -I spike/pitch_changes.py notes.json feat.npz lyr_mms.json   # pitch crossings
```

Details that matter: recording time = music time × (score tempo ÷ `% BPM:`)
− `% Audio offset:` (D155). LilyPond writes the lyrics on a track of their
own, at their notes' ticks. `torchaudio.load` needs torchcodec in 2.11, so the
scripts load with librosa. MMS_FA has no space token, so each syllable is
aligned as a word, with `<star>` between syllables.

## Appendix B: sources

- Forced alignment: [torchaudio forced_align and its un-deprecation](https://github.com/pytorch/audio/issues/3902), [ctc-forced-aligner](https://github.com/MahmoudAshraf97/ctc-forced-aligner), [WhisperX](https://github.com/m-bain/whisperX), [jonatasgrosman/wav2vec2-large-xlsr-53-russian](https://huggingface.co/jonatasgrosman/wav2vec2-large-xlsr-53-russian), [NeMo Forced Aligner](https://docs.nvidia.com/nemo-framework/user-guide/latest/nemotoolkit/tools/nemo_forced_aligner.html), [Montreal Forced Aligner](https://github.com/MontrealCorpusTools/Montreal-Forced-Aligner), [MFA models](https://mfa-models.readthedocs.io), [stable-ts](https://github.com/jianfch/stable-ts)
- Lyrics alignment research: [Durand, Stoller & Ewert 2023](https://arxiv.org/abs/2306.07744), [JamendoLyrics](https://huggingface.co/datasets/jamendolyrics/jamendolyrics), [Kick et al. 2024](https://github.com/tikick/LyricsAlignment), [Huang, Benetos & Ewert 2022](https://arxiv.org/abs/2202.01646) and [code](https://github.com/jhuang448/LyricsAlignment-MTL), [AutoLyrixAlign](https://github.com/chitralekha18/AutoLyrixAlign), [schufo lyrics-aligner](https://github.com/schufo/lyrics-aligner), [Cheng, Nakano & Goto, DAFx 2025](https://www.dafx.de/paper-archive/details/o-wYFu7sQFE_GuYrAHxIHA), [real-time alignment for classical voice](https://arxiv.org/abs/2401.09200), DeepSinger ([arXiv 2007.04590](https://arxiv.org/abs/2007.04590))
- Score-to-audio alignment: [Peter, Hu & Widmer 2026](https://arxiv.org/abs/2605.20014), [parangonar](https://github.com/sildater/parangonar), [partitura](https://github.com/CPJKU/partitura), [SOLEA, SMC 2025](https://zenodo.org/records/15838731) and [code](https://github.com/migperfer/solea), [Murgul et al. 2025](https://arxiv.org/abs/2506.22237), [synctoolbox](https://github.com/meinardmueller/synctoolbox), [pyAMPACT](https://arxiv.org/abs/2412.05436) and [code](https://github.com/pyampact/pyAMPACT), [Nakamura's alignment tool](https://midialignment.github.io/demo.html)
- Singing transcription: [SOFA](https://github.com/qiuqiao/SOFA) and [its model discussions](https://github.com/qiuqiao/SOFA/discussions/31), [SOFA English models](https://github.com/spicytigermeat/SOFA-Models/releases/tag/v1.0.0_en), [SOFA Russian models](https://github.com/Megageorgio/SOFA-Models), [HubertFA](https://github.com/wolfgitpr/HubertFA), [GAME](https://github.com/openvpi/GAME) and [its ALGORITHMS.md](https://github.com/openvpi/GAME/blob/main/ALGORITHMS.md), [SOME](https://github.com/openvpi/SOME), [MakeDiffSinger](https://github.com/openvpi/MakeDiffSinger), [dataset-tools](https://github.com/openvpi/dataset-tools), [ROSVOT](https://arxiv.org/abs/2405.09940) and [code](https://github.com/RickyL-2000/ROSVOT), [STARS](https://arxiv.org/abs/2507.06670) and [code](https://github.com/gwx314/STARS), [VocalParse](https://arxiv.org/abs/2605.04613), [T3MS](https://arxiv.org/abs/2502.12438)
- Pitch: [pitch-benchmark](https://github.com/lars76/pitch-benchmark), [SwiftF0](https://github.com/lars76/swift-f0), [RMVPE](https://arxiv.org/abs/2306.15412), [FCPE](https://github.com/CNChTu/FCPE), [PESTO](https://github.com/SonyCSLParis/pesto), [torchcrepe](https://github.com/maxrmorrison/torchcrepe)
