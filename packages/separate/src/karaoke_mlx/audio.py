"""Audio decode / encode through ffmpeg (any format in, wav/flac/mp3 out)."""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import numpy as np

ENCODERS = {
    "wav": ["-c:a", "pcm_s16le"],
    "flac": ["-c:a", "flac"],
    "mp3": ["-c:a", "libmp3lame", "-b:a", "320k"],
}


def _ffmpeg() -> str:
    for candidate in (shutil.which("ffmpeg"), "/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"):
        if candidate and Path(candidate).exists():
            return candidate
    raise RuntimeError("ffmpeg not found; install it with `brew install ffmpeg`")


def load(path: str | Path, sample_rate: int = 44100) -> np.ndarray:
    """Decode any audio file to float32 stereo, shape (2, samples)."""
    cmd = [_ffmpeg(), "-v", "error", "-nostdin", "-i", str(path), "-f", "f32le", "-ac", "2", "-ar", str(sample_rate), "-"]
    proc = subprocess.run(cmd, capture_output=True)
    if proc.returncode != 0:
        raise RuntimeError(f"could not decode {path}: {proc.stderr.decode(errors='replace').strip()}")
    return np.frombuffer(proc.stdout, dtype=np.float32).reshape(-1, 2).T.copy()


def save(path: str | Path, audio: np.ndarray, sample_rate: int = 44100) -> None:
    """Encode (channels, samples) float audio; format follows the file extension."""
    path = Path(path)
    codec = ENCODERS[path.suffix.lstrip(".").lower()]
    data = np.ascontiguousarray(np.clip(audio, -1.0, 1.0).T, dtype=np.float32)
    cmd = [
        _ffmpeg(), "-v", "error", "-nostdin", "-y",
        "-f", "f32le", "-ac", str(audio.shape[0]), "-ar", str(sample_rate), "-i", "-",
        *codec, str(path),
    ]
    proc = subprocess.run(cmd, input=data.tobytes(), capture_output=True)
    if proc.returncode != 0:
        raise RuntimeError(f"could not write {path}: {proc.stderr.decode(errors='replace').strip()}")
