# From a recording to a score

How to turn an audio track and the name of the instrument playing it into
a LilyPond score: what the tools and models available in October 2026 do
at each step, what the instrument's name is good for, what a spike on our
own machine showed, and in what order the work could be done. Nothing here
is decided. A decision in `docs/DECISIONS.md` should follow if the work goes
ahead.

**[measured]** marks what was run on 2026-10-14 on the development Mac with
`GNU LilyPond 2.26.0` from Homebrew. The recipe is in Appendix A. Every
other claim comes from the papers and READMEs listed in Appendix B, and a
leaderboard number from one paper should not be compared with a number
from another.

## 1. The problem in one picture

No model available today goes from audio straight to an engraved,
multi-voice score of any instrument. Products that claim to do so
(Songscription, Klangio, Musirion) and research systems alike chain
separate stages. Each stage answers a different question:

```
audio ──(1) separation───▶ the instrument's own track      only for a mix
      ──(2) transcription─▶ notes in seconds: pitch, onset, offset, loudness
      ──(3) beat tracking─▶ beats and downbeats in seconds
      ──(4) quantization──▶ notes in beats: values, bars, ties, tuplets
      ──(5) notation──────▶ key, spelling, clef, voices, staves, articulations
      ──(6) writing───────▶ a .ly file that Lily Studio opens and engraves
```

Steps 2 and 3 are machine learning over audio. Steps 4 and 5 are mostly
rules, and the research on doing them with models exists only for piano
(§3.4). Step 6 is ours. The spike (§4) shows that step 2 is now good on a
clean solo recording. The hard part is steps 3 to 5: where the bar lines go,
what the notes are worth, and how they are spelled.

## 2. What the instrument's name is for

The name is not just a label. At almost every step it narrows the answer:

| Step | What the name decides | Example |
| --- | --- | --- |
| 1 Separation | which stem to keep, or whether separation is needed at all | "violin" in a band mix: a strings stem. A solo violin recording: none |
| 2 Transcription | which model to use. Conditioning, if the model accepts it | monophonic (flute, voice, trumpet): a pitch tracker or Basic Pitch. Piano: a piano model. In a mix: MuScriptor with "violin" as the instrument present |
| 2 Cleanup | the sounding range. Notes outside it are errors | the spike's stray F♯1 under a violin (§4) |
| 4 Quantization | how many notes can sound at once | flute 1, violin ≤ 4 (double stops), guitar ≤ 6, piano 10 |
| 5 Notation | clef, transposition, staves, idioms | cello: bass/tenor clefs. B♭ clarinet: written a major 2nd higher. Guitar: `treble_8` and tablature. Piano: two staves and a hand split. Drums: a percussion staff and a drum map |
| 6 Writing | `\instrumentName`, `midiInstrument`, `\transpose` | playback in Lily Studio uses the right sound |

So the instrument should be chosen from a catalogue in which each entry has a
range, a transposition, a clef, a maximum polyphony and a family (monophonic,
keyboard, fretted, percussion, voice), and a `midiInstrument` name for
playback. It should not be typed as free text.

## 3. The steps, and the best tools for each

### 3.1 Separation (only when the track is a mix)

- **HT-Demucs 6-stem** (MIT): vocals, drums, bass, guitar, piano, other. An
  ONNX export exists. It is good on vocals, drums and bass. Guitar and piano
  are weaker because they overlap in frequency with everything else.
- **BS-RoFormer / Mel-RoFormer** family: these lead MVSep's public
  leaderboards in 2026. A 53-stem community model (MVSep "Mega", reportedly
  MIT, ≥ 16 GB of GPU memory) separates strings, brass and winds. Its users
  report that a single violin is still hard to isolate cleanly.

Separation adds artifacts that the transcriber then reads as notes. So when
a model transcribes the whole mix and accepts the instrument as a condition
(§3.2), skipping separation may be better.

### 3.2 Transcription: audio to notes in seconds

| Model | What it does | Licence | Fit for us |
| --- | --- | --- | --- |
| **Basic Pitch** (Spotify, 2022) | instrument-agnostic, polyphonic, notes with pitch bends. Best on one instrument at a time | Apache-2.0. The ONNX model is **230 KB [measured]**. There is a TypeScript port (`basic-pitch-ts`) | the default for a solo track. Small enough to run in Rust through `ort`, or in the browser |
| **MuScriptor** (Kyutai and Mirelo, July 2026) | a decoder-only transformer (100M / 300M / 1.4B parameters) over 5-second windows that outputs notes for each of 36 instrument classes. Can be **conditioned on the instruments present**. Trained on 11k hours of real recordings, then post-trained with RL | code MIT, **weights CC BY-NC 4.0** | the best open model for a mix by its own paper (Multi F1 47.8 against YourMT3+'s 21.9 on the authors' test set, not yet replicated). Lily is MIT and could be sold, so the weights cannot be bundled without a licence from the authors. Its CLI already writes MusicXML and PDF through MuseScore 4 (`--format sheets`) |
| **YourMT3+** (2024) | MT3-family multi-instrument model. Transcribes singing alongside other instruments | GPL-3.0 | strong on benchmarks (Slakh2100 note F1 0.85, MAESTRO 96.5), but GPL code cannot be linked into an MIT app. Usable only as a separate process |
| **Harmonica** (September 2026) | instrument-agnostic like Basic Pitch, from 26.3K parameters ("nano", 1,622× real time) up. The authors report frame F1 14.6 points above Basic Pitch | no code or weights found yet | worth watching as Basic Pitch's successor |
| Pitch trackers: **PESTO**, CREPE, RMVPE, FCPE | a frame-wise f0 for one voice. Notes still have to be cut out of it (onset and offset detection) | various | for voice and monophonic winds, where vibrato and glides confuse note models. RMVPE finds the vocal line inside a mix |

Piano has its own, more accurate models (MAESTRO-trained, onset F1 above
0.95), and guitar has audio-to-tab research (TART, 2025). Neither changes
the shape of the pipeline.

### 3.3 Beat tracking: where the beats and bars are

**Beat This!** (CPJKU, ISMIR 2024, MIT) is a transformer that outputs beats
and downbeats without the older DBN postprocessing (the madmom DBN is
licensed for non-commercial use only). In the spike it found every beat of
a rubato performance, but **not the downbeats** (§4). On a melody without
accents nothing in the audio says where the bar starts. Meter is where a
transcription tool should expect to ask the user.

### 3.4 Quantization and notation: notes in seconds to a score

This is where commercial tools differ most, and where open tools are thinnest.

- **Rules** (what the spike does): map every onset and offset through the
  beat grid into beats, snap to a subdivision, drop notes shorter than the
  grid, enforce the instrument's polyphony, fill legato gaps, then let a
  key-finding algorithm (Krumhansl–Schmuckler in music21) choose the key.
  This is enough for a steady solo line.
- **PM2S** (QMUL, ISMIR 2022): a CRNN that predicts beats, quantized onsets,
  note values, key, time signature and hand parts from performance MIDI.
  It writes MIDI, MusicXML or LilyPond. Piano only.
- **MIDI2ScoreTransformer** (Beyer and Dai, ISMIR 2024): seq2seq from
  performance MIDI to a score, including staff assignment, stem direction
  and trills. It beats HMM pipelines on MUSTER. Piano only. Code and models
  are on GitHub.
- **Beat-aware quantization** (Wachter et al., 2025): a T5 model that takes
  notes plus a beat grid, from a tracker or a metronome. Not tied to one
  time signature.
- **End-to-end audio-to-score**: CNN/transformer models that output
  `**kern` directly (Alfaro-Contreras 2024, and the SheetSage-A2S model of
  2026 with a frozen MuQ encoder). Symbol error rate is 5 % on synthesized
  string quartets but 21 % on real pop recordings (melody plus chords). This
  is research, not a tool.
- **Tablature**: Fretting-Transformer (ICMC 2025) and MIDI-to-Tab (ISMIR
  2024) assign strings and frets to known pitches, so they can sit after any
  transcriber.

### 3.5 Writing LilyPond

LilyPond ships two converters. Neither is enough on its own (§4):

- `midi2ly` quantizes against the MIDI file's own tempo. A transcriber does
  not know the tempo and writes 120 bpm, so the grid is wrong and the
  rhythms come out dotted and shifted.
- `musicxml2ly` converts faithfully, but what it gets from music21 is noisy:
  a forced natural on every white-key note, a "Music21 Fragment" title, and
  hashed variable names.

Writing the `.ly` text ourselves from the quantized notes is little code
(durations, ties across bar lines, `\relative` pitches in the key's
spelling). It yields a score a person would have typed, and it is the format
Lily Studio's editor and agents already work in.

## 4. The spike

**[measured]** The first 12 bars of *Ode to Joy* (47 notes, D major, 4/4,
♩ = 100, a dotted figure and eighths) were written in LilyPond, rendered to
MIDI, and synthesized as a violin-like tone: a sawtooth with vibrato, a bow
attack and noise. One version was played exactly. The other was played like
a person, with ±4 % rubato over 8 s and 12 ms of onset jitter. Then:

| Stage | Result |
| --- | --- |
| Basic Pitch, exact version | 49 notes. Onset F1 0.98 (mir_eval, 50 ms). Every melody pitch right, plus two extras at the end: **F♯1** (below the violin's G3) and a short E4 |
| Basic Pitch, human version | the same 49 pitches in the same order. Its onsets follow the rubato, as they should |
| Basic Pitch time | 4.7 s wall for 33 s of audio, Python start-up included, CoreML |
| `midi2ly -k 2 -d 8 -s 8` on the human version | wrong: at the assumed 120 bpm the grid does not fit, e.g. `fis4. fis4 g4.` |
| Beat This (CPU), human version | 48 beats, which is the right number, about 0.6 s apart (100 bpm). Downbeats irregular (15 for 12 bars) |
| Our quantizer + violin range | F♯1 dropped by range, E4 dropped by length. 47 notes written. Key D major (r = 0.83). ♩ = 100 |
| Bar phase from the downbeats' vote | **wrong by half a bar**: every bar line in the middle of a bar |
| Bar phase given by hand (first note on beat 1) | **every pitch and duration of all 12 bars right**, the dotted quarter and eighth pairs included |
| `musicxml2ly` → `lilypond` | engraves, with the cosmetic noise of §3.5 and a double-dotted final note where the last half note met the end of the audio |

What it shows:

1. On a clean solo line, transcription is solved well enough, even by a
   230 KB model. The instrument's range removes its worst error.
2. Beat tracking turns performance timing into note values reliably, even
   with rubato. That is what `midi2ly` alone cannot do.
3. **Downbeat and meter are the weak point.** A wrong bar phase gives a
   score that looks plausible and is wrong everywhere. The tool should
   suggest a meter and phase, show them, and let the user correct them with
   one click ("the first note is on beat …").
4. Synthesized audio is the easy case. Real recordings (reverb, bow noise,
   breath, a piano's sustain pedal) will need a test set: URMP has real
   recordings of each instrument, made separately, with aligned scores.

## 5. How it could fit Lily

**Where it runs.** Basic Pitch (230 KB ONNX) and a small quantizer and
writer in Rust can run inside Lily Studio on any Mac, and in the browser if
need be. Beat This is a PyTorch model of 77 MB (the checkpoint used here).
It would need an ONNX export to go the same way, or it can run on the
server of `lily-studio-server`, in the user's sandbox (D55). Large models
(MuScriptor, separation) belong on the server or in an optional download,
never in the app bundle.

**Licences.** Basic Pitch (Apache-2.0), Beat This (MIT), Demucs (MIT) and
music21 (BSD) are compatible with an MIT app. MuScriptor's weights
(CC BY-NC) and YourMT3+ (GPL-3.0) are not, unless we get permission or run
them as a separate tool that the user installs.

**What the user sees.** A command *File › New Score from Recording…*:
choose the audio file and the instrument, then confirm tempo, meter and
first downbeat on a waveform with the detected beats drawn over it. The
result is a new `.ly` file, engraved at once. Lily Studio's player can play
it next to the original audio, which is the fastest way to hear what went
wrong. Lily's agents can then fix the rest in text, which is where a
transcription's errors are cheapest to correct.

**Suggested order.**

1. *Solo, monophonic* (flute, violin, voice, trumpet, cello): Basic Pitch →
   Beat This → a rule-based quantizer using the instrument catalogue (§2) →
   our own `.ly` writer, with the meter and downbeat confirmed by the user.
   Measure note F1 and MUSTER on URMP and on our own scores rendered to
   audio.
2. *Piano*: a piano transcription model, then PM2S or MIDI2ScoreTransformer
   for note values, voices and the hand split. These are the only learned
   quantizers available, and only for piano.
3. *A track from a mix*: separation, or MuScriptor conditioned on the
   instrument if its licence allows. Then the same back half as 1 or 2.
4. *Guitar*: tablature through a string and fret model, written as
   `TabStaff`.

Expressive marks (dynamics from velocity, slurs, articulations, bends) can
come last. They are what transcription gets wrong most, and they are the
easiest for a person to add.

## Appendix A — recipe of the spike

The scripts were written in a scratch directory and are not part of the
repository. Rebuild them from this description. Python 3.11 is needed,
because Basic Pitch does not support 3.12 or later. The tools were run
through `uv`.

```sh
lilypond ode.ly                                   # ode.midi (violin, ♩=100)
uv run --python 3.11 --with basic-pitch --with soundfile \
  python synth.py ode.midi ode_human.wav          # sawtooth + vibrato, ±4 % rubato, 12 ms jitter
uv run --python 3.11 --with basic-pitch basic-pitch --save-note-events bp ode_human.wav
uv run --python 3.11 --with pretty_midi --with mir_eval \
  python evalnotes.py ode.midi bp/ode_human_basic_pitch.mid
midi2ly -k 2 -d 8 -s 8 bp/ode_human_basic_pitch.mid
uv run --python 3.11 --with "beat_this @ https://github.com/CPJKU/beat_this/archive/main.zip" \
  --with soundfile --with soxr --with music21 --with pretty_midi \
  python toscore.py ode_human.wav bp/ode_human_basic_pitch.mid violin ode.musicxml 4 0
musicxml2ly ode.musicxml && lilypond ode.ly
```

`toscore.py` does the following: Beat This (`final0`, CPU, no DBN) finds
the beats, extended by one beat on each side. Note times are interpolated
onto beat indices and rounded to sixteenths. Notes outside the violin's
range (G3–E7) or shorter than a sixteenth are dropped. Of two notes with the
same onset, the louder one is kept. A note lasts until the next one unless
the gap is at least half a beat. The last argument sets the bar phase.
Without it, the phase comes from a vote of the detected downbeats. music21
then analyzes the key, makes the measures and ties, and writes MusicXML.

## Appendix B — sources

- MuScriptor: [paper](https://arxiv.org/html/2607.08168v1), [code](https://github.com/muscriptor/muscriptor), [Mirelo's post](https://mirelo.ai/blog/turning-audio-to-midi)
- [2025 AMT Challenge results](https://arxiv.org/pdf/2603.27528): two of eight teams beat MT3
- [YourMT3+](https://arxiv.org/pdf/2407.04822), [code](https://github.com/mimbres/YourMT3); benchmark tables on [Papers-with-Code mirror (Slakh2100)](https://hyper.ai/en/sota/tasks/music-transcription/benchmark/music-transcription-on-slakh2100)
- [Basic Pitch](https://github.com/spotify/basic-pitch)
- [Harmonica](https://arxiv.org/abs/2609.04640)
- [PESTO](https://arxiv.org/pdf/2508.01488), [RMVPE](https://arxiv.org/pdf/2306.15412), [FCPE](https://arxiv.org/html/2509.15140v1)
- [Beat This!](https://arxiv.org/pdf/2407.21658)
- [PM2S](https://ismir2022program.ismir.net/poster_335.html), [MIDI2ScoreTransformer](https://arxiv.org/abs/2410.00210) ([code](https://github.com/TimFelixBeyer/MIDI2ScoreTransformer)), [beat-aware quantization](https://arxiv.org/pdf/2508.19262)
- [SheetSage-A2S audio-to-score](https://arxiv.org/html/2608.06165), [end-to-end piano audio-to-score](https://arxiv.org/abs/2405.13527)
- [TART audio-to-tab](https://arxiv.org/html/2510.02597v1), [MIDI-to-Tab](https://arxiv.org/html/2408.05024v1)
- Separation: [HT-Demucs 6-stem ONNX](https://huggingface.co/adowu/htdemucs-6s-onnx), [MVSep 53-stem model thread](https://vi-control.net/community/threads/new-open-source-53-stems-audio-splitting-model-available.171774/)
- Products: [Songscription](https://songscription.ai), [Klangio](https://klang.io/transcription-plugin/)
