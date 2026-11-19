"""Chunked, overlap-added inference over a whole track."""

from __future__ import annotations

from typing import Callable

import mlx.core as mx
import numpy as np

from .model import Roformer
from .models import ModelSpec


def _fade_window(size: int, fade: int, fade_in: bool, fade_out: bool) -> np.ndarray:
    window = np.ones(size, dtype=np.float32)
    if fade_in:
        window[:fade] = np.linspace(0, 1, fade)
    if fade_out:
        window[-fade:] = np.linspace(1, 0, fade)
    return window


def separate(
    model: Roformer,
    spec: ModelSpec,
    mix: np.ndarray,
    num_overlap: int | None = None,
    batch_size: int = 1,
    progress: Callable[[float], None] | None = None,
) -> dict[str, np.ndarray]:
    """Split a (2, samples) float32 mix into ``{stem: (2, samples)}`` in ``spec.outputs`` order.

    Same scheme as MSST's ``demix``: reflect-pad the track, slide a
    ``chunk_size`` window by ``chunk_size / num_overlap``, and blend the
    chunks with linear cross-fades.
    """
    chunk = spec.chunk_size
    step = chunk // (num_overlap or spec.num_overlap)
    fade = chunk // 10
    border = chunk - step
    length = mix.shape[-1]

    padded = length > 2 * border and border > 0
    x = np.pad(mix, ((0, 0), (border, border)), mode="reflect") if padded else mix
    total = x.shape[-1]

    starts = list(range(0, total, step))
    result = np.zeros((len(spec.stems), *x.shape), dtype=np.float32)
    weight = np.zeros(total, dtype=np.float32)

    for b in range(0, len(starts), batch_size):
        batch_starts = starts[b : b + batch_size]
        parts = []
        for start in batch_starts:
            part = x[:, start : start + chunk]
            seg = part.shape[-1]
            if seg < chunk:
                mode = "reflect" if seg > chunk // 2 else "constant"
                part = np.pad(part, ((0, 0), (0, chunk - seg)), mode=mode)
            parts.append(part)

        out = model(mx.array(np.stack(parts)))
        out = np.array(out.astype(mx.float32))  # (batch, stems, channels, chunk)

        for j, start in enumerate(batch_starts):
            seg = min(chunk, total - start)
            window = _fade_window(chunk, fade, fade_in=start > 0, fade_out=start + step < total)[:seg]
            result[..., start : start + seg] += out[j, ..., :seg] * window
            weight[start : start + seg] += window

        if progress:
            progress(min(1.0, (b + len(batch_starts)) / len(starts)))

    result /= np.maximum(weight, 1e-8)
    if padded:
        result = result[..., border:-border]
    result = np.nan_to_num(result)

    stems = dict(zip(spec.stems, result))
    if spec.residual:
        stems[spec.residual] = mix - sum(stems.values())
    return {name: stems[name] for name in spec.outputs}
