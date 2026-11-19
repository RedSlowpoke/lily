# karaoke-mlx

Lily Studio's stem separation (DECISIONS D112, D125): RoFormer models running
natively on Apple Silicon with [MLX](https://github.com/ml-explore/mlx).
*Karaoke* splits a song into lead vocals and instrumental, *All instruments*
into six stems. Lily Studio runs it with [uv](https://docs.astral.sh/uv/) as a
sidecar, `uv run karaoke-mlx --json separate …`, one JSON event per line.

The models are a from-scratch MLX port of `MelBandRoformer` and `BSRoformer`
from
[ZFTurbo/Music-Source-Separation-Training](https://github.com/ZFTurbo/Music-Source-Separation-Training).
They load the public PyTorch checkpoints directly, with no torch at run time,
and match the reference implementation to about 1e-5 in float32.

| id | task | model | output |
| --- | --- | --- | --- |
| `becruily` (default) | karaoke | [Mel-RoFormer Karaoke by becruily](https://huggingface.co/becruily/mel-band-roformer-karaoke) | lead vocals + instrumental (backing vocals kept) |
| `aufr33-viperx` | karaoke | Mel-RoFormer Karaoke by aufr33 & viperx (SDR 10.19) | lead vocals; instrumental = mix − lead |
| `bs-roformer-sw` | stems | BS-RoFormer SW by jarredou | vocals, drums, bass, guitar, piano, other |

Each model downloads on its first use (1.7 GB / 0.9 GB / 0.7 GB), is SHA-256
checked and converted to `~/.cache/karaoke-mlx/models/<id>/model.safetensors`;
the original `.ckpt` is deleted. `KARAOKE_MLX_HOME` keeps them elsewhere.

It needs an Apple Silicon Mac, uv and ffmpeg (`brew install uv ffmpeg`).

```sh
uv sync                                                   # once
uv run karaoke-mlx models
uv run karaoke-mlx separate song.mp3                      # -> "song (karaoke)/song - instrumental.wav", "... - vocals.wav"
uv run karaoke-mlx separate song.mp3 -m bs-roformer-sw    # -> "song (stems)/song - vocals.wav", "... - drums.wav", ...
uv run karaoke-mlx separate song.mp3 -m aufr33-viperx --overlap 2 -f mp3 -o out/
uv run karaoke-mlx mix "song (stems)/song - drums.wav" "song (stems)/song - bass.wav" -o rhythm.mp3
uv run pytest -s
```

`--overlap` is the main speed and quality knob: how many chunks cover each
sample, 2 fast, 4 balanced, 8 best. `--dtype float32` is a little closer to
PyTorch; `float16`, the default, is about 20 % faster. On the development Mac a song of
3:43 takes about 90 s with becruily at overlap 4, 3 min at overlap 8, 75 s
with aufr33-viperx at overlap 4 and 55 s with bs-roformer-sw at overlap 2.

The tests compare the mel band layout with librosa, the STFT and iSTFT with
torch, and every model with the PyTorch reference and the real weights; a
model not yet downloaded is skipped. The dev group brings torch and librosa;
the run time needs only `mlx` and `numpy`.

```
src/karaoke_mlx/
  model.py       MLX Mel-/BS-RoFormer, STFT/iSTFT, the mel band layout
  checkpoint.py  a torch .ckpt reader without torch (restricted unpickler)
  models.py      the models: download, verify, convert, load
  separate.py    chunked overlap-add inference over a whole track
  audio.py       ffmpeg decode and encode
  cli.py         the `karaoke-mlx` command
tests/           parity tests; tests/reference is ZFTurbo's PyTorch code (MIT)
```

The code is MIT. The weights belong to their authors (becruily; aufr33 &
viperx; jarredou): check their terms before redistributing them.
