# lily-transcribe

Lily Studio's transcription of a stem (DECISIONS D113): the notes, beats,
meter and key that *New score for bass* writes a score from. Lily Studio runs
it with [uv](https://docs.astral.sh/uv/) as a sidecar, as it runs RoFormer
MLX's separation (D112).

```sh
uv sync                    # once; installs Python 3.11, Basic Pitch, Beat This and torch
uv run lily-transcribe transcribe "song (stems)/song - bass.wav" --instrument bass
uv run lily-transcribe tempo "song - instrumental.mp3" --score-bpm 114 --beats-per-bar 4
uv run pytest
```

- Notes: [Basic Pitch](https://github.com/spotify/basic-pitch) (Apache-2.0).
- Beats and downbeats, from all the song's stems mixed back together:
  [Beat This!](https://github.com/CPJKU/beat_this) (MIT, code and weights),
  without madmom's DBN, which is for non-commercial use only. Its checkpoint
  (77 MB) downloads on the first transcription.
- Kick, snare and hi-hat, the key, the grid and the quantizing: this package.
- Words, for vocals (D134): Whisper large-v3-turbo on MLX
  ([mlx-whisper](https://github.com/ml-explore/mlx-examples), MIT, code and
  weights) hears them with their times; [Pyphen](https://github.com/Kozea/Pyphen)'s
  hyphenation dictionaries split them into syllables; this package lays the
  syllables on the notes. The model (1.6 GB) downloads on the first vocals;
  `LILY_TRANSCRIBE_WHISPER` names another.

`tempo` (D159) is Lily Studio's *From the recording*: a score's tempo and
bar 1 from its `% Audio:` recording, for its `% BPM:` and `% Audio offset:`.
Beat This runs on the recording itself, with nothing mixed in. The tempo is a
straight line through the beats, kept to the hundredth (no snap to a whole
number, which is for a `\tempo`), at the metrical level (×½, ×1, ×2, and ×⅔,
×3⁄2 for a dotted beat) nearest `--score-bpm` (120 without one), in quarter
notes a minute: `--beat 1.5` says the score counts 6/8's dotted quarters.
`--beats-per-bar` groups the beats into the score's bars; the downbeats only
choose which beat starts one. `done` has `bpm`, `heardBpm`, `steady`, `share`
(of the beats on one tempo), `residualMs` (their 95th percentile from it),
`beatsPerBar`, `heardBeatsPerBar`, `barOne` (the first bar line at or after
the start, in seconds) and `seconds` (the recording's length). A recording
with too few beats, or beats that keep no tempo, as a voice alone, ends in
`error`.

The beats and key of a song are kept in `~/.cache/lily-transcribe`
(`LILY_TRANSCRIBE_HOME`), so the song's other stems are quicker; so are a
recording's beats, so `tempo` is instant the second time.
