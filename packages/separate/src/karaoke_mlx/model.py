"""Mel-Band RoFormer and BS-RoFormer in MLX.

A port of the inference path of ``MelBandRoformer`` and ``BSRoformer`` from
ZFTurbo's Music-Source-Separation-Training (based on lucidrains' BS-RoFormer).
The two differ only in how STFT bins are grouped into bands: mel bands that
overlap (masks are averaged where they do) vs. fixed contiguous bands.
Modules are laid out so that their parameter names match the PyTorch
``state_dict`` one to one (``nn.Sequential`` indices become list indices), so
converted checkpoints load without any key remapping.
"""

from __future__ import annotations

import math
from dataclasses import dataclass

import mlx.core as mx
import mlx.nn as nn
import numpy as np


@dataclass(frozen=True)
class ModelConfig:
    dim: int = 384
    depth: int = 6
    stereo: bool = True
    num_stems: int = 1
    time_transformer_depth: int = 1
    freq_transformer_depth: int = 1
    num_bands: int = 60
    dim_head: int = 64
    heads: int = 8
    sample_rate: int = 44100
    stft_n_fft: int = 2048
    stft_hop_length: int = 441
    stft_win_length: int = 2048
    mask_estimator_depth: int = 2
    mlp_expansion_factor: int = 4
    zero_dc: bool = True
    # set for BS-RoFormer: STFT bins per band, contiguous; None means mel bands
    freqs_per_bands: tuple[int, ...] | None = None

    @property
    def is_mel(self) -> bool:
        return self.freqs_per_bands is None


# --- mel band layout ---------------------------------------------------------


def _hz_to_mel(f):
    # librosa's Slaney mel scale: linear below 1 kHz, logarithmic above.
    f = np.asarray(f, dtype=np.float64)
    f_sp, min_log_hz = 200.0 / 3, 1000.0
    min_log_mel, logstep = min_log_hz / f_sp, math.log(6.4) / 27.0
    return np.where(f >= min_log_hz, min_log_mel + np.log(np.maximum(f, 1e-10) / min_log_hz) / logstep, f / f_sp)


def _mel_to_hz(m):
    m = np.asarray(m, dtype=np.float64)
    f_sp, min_log_hz = 200.0 / 3, 1000.0
    min_log_mel, logstep = min_log_hz / f_sp, math.log(6.4) / 27.0
    return np.where(m >= min_log_mel, min_log_hz * np.exp(logstep * (m - min_log_mel)), f_sp * m)


def mel_band_mask(sample_rate: int, n_fft: int, num_bands: int) -> np.ndarray:
    """Boolean (num_bands, n_fft // 2 + 1) map of which STFT bins feed each band.

    Mirrors ``librosa.filters.mel(sr, n_fft, n_mels)`` > 0, plus the two
    forced corners the reference model sets so every bin is covered.
    """
    fft_freqs = np.fft.rfftfreq(n_fft, d=1.0 / sample_rate)
    mel_f = _mel_to_hz(np.linspace(_hz_to_mel(0.0), _hz_to_mel(sample_rate / 2), num_bands + 2))
    fdiff = np.diff(mel_f)
    ramps = np.subtract.outer(mel_f, fft_freqs)
    weights = np.zeros((num_bands, len(fft_freqs)), dtype=np.float32)
    for i in range(num_bands):
        lower = -ramps[i] / fdiff[i]
        upper = ramps[i + 2] / fdiff[i + 1]
        weights[i] = np.maximum(0, np.minimum(lower, upper))
    weights *= (2.0 / (mel_f[2 : num_bands + 2] - mel_f[:num_bands]))[:, None]
    weights[0, 0] = 1.0
    weights[-1, -1] = 1.0
    mask = weights > 0
    assert mask.any(axis=0).all(), "every frequency bin must belong to at least one band"
    return mask


# --- STFT ----------------------------------------------------------------------


def hann_window(n: int) -> np.ndarray:
    # periodic Hann, as torch.hann_window
    return (0.5 - 0.5 * np.cos(2 * np.pi * np.arange(n) / n)).astype(np.float32)


def stft(x: mx.array, n_fft: int, hop: int, window: mx.array) -> mx.array:
    """(B, T) float -> (B, frames, n_fft // 2 + 1) complex; torch.stft(center=True, reflect)."""
    pad = n_fft // 2
    x = mx.concatenate([x[:, 1 : pad + 1][:, ::-1], x, x[:, -pad - 1 : -1][:, ::-1]], axis=1)
    batch, length = x.shape
    frames = 1 + (length - n_fft) // hop
    x = mx.as_strided(x, shape=(batch, frames, n_fft), strides=(length, hop, 1))
    return mx.fft.rfft(x * window, axis=-1)


def istft(spec: mx.array, n_fft: int, hop: int, window: mx.array, length: int | None = None) -> mx.array:
    """Inverse of :func:`stft`, as torch.istft(center=True).

    Returns ``length`` samples (zero-padded past the last frame if needed), or
    hop * (frames - 1) samples when ``length`` is None.
    """
    batch, frames, _ = spec.shape
    chunks = mx.fft.irfft(spec, n=n_fft, axis=-1) * window
    total = n_fft + hop * (frames - 1)
    index = (mx.arange(frames)[:, None] * hop + mx.arange(n_fft)[None, :]).reshape(-1)
    out = mx.zeros((batch, total), dtype=chunks.dtype).at[:, index].add(chunks.reshape(batch, -1))

    win_sq = np.square(np.asarray(window, dtype=np.float64))
    envelope = np.zeros(total)
    for f in range(frames):
        envelope[f * hop : f * hop + n_fft] += win_sq
    pad = n_fft // 2
    end = min(total, pad + length) if length is not None else total - pad
    envelope = mx.array(envelope[pad:end].astype(np.float32))
    wav = out[:, pad:end] / mx.maximum(envelope, 1e-11)
    if length is not None and wav.shape[-1] < length:
        wav = mx.pad(wav, [(0, 0), (0, length - wav.shape[-1])])
    return wav


# --- layers --------------------------------------------------------------------


class RMSNorm(nn.Module):
    def __init__(self, dim: int):
        super().__init__()
        self.gamma = mx.ones((dim,))

    def __call__(self, x):
        # F.normalize(x) * sqrt(dim) * gamma == x / rms(x) * gamma
        return mx.fast.rms_norm(x, self.gamma, eps=1e-12)


class Attention(nn.Module):
    def __init__(self, dim: int, heads: int, dim_head: int):
        super().__init__()
        self.heads = heads
        self.dim_head = dim_head
        self.norm = RMSNorm(dim)
        self.to_qkv = nn.Linear(dim, heads * dim_head * 3, bias=False)
        self.to_gates = nn.Linear(dim, heads)
        self.to_out = [nn.Linear(heads * dim_head, dim, bias=False)]

    def __call__(self, x):
        b, n, _ = x.shape
        x = self.norm(x)
        qkv = self.to_qkv(x).reshape(b, n, 3, self.heads, self.dim_head).transpose(2, 0, 3, 1, 4)
        q, k, v = qkv[0], qkv[1], qkv[2]
        # rotary_embedding_torch rotates interleaved pairs == MLX "traditional" RoPE
        q = mx.fast.rope(q, self.dim_head, traditional=True, base=10000.0, scale=1.0, offset=0)
        k = mx.fast.rope(k, self.dim_head, traditional=True, base=10000.0, scale=1.0, offset=0)
        out = mx.fast.scaled_dot_product_attention(q, k, v, scale=self.dim_head**-0.5)
        gates = mx.sigmoid(self.to_gates(x)).transpose(0, 2, 1)[..., None]
        out = (out * gates).transpose(0, 2, 1, 3).reshape(b, n, -1)
        return self.to_out[0](out)


class FeedForward(nn.Module):
    def __init__(self, dim: int, mult: int = 4):
        super().__init__()
        # indices mirror the torch Sequential: norm, linear, gelu, dropout, linear
        self.net = [RMSNorm(dim), nn.Linear(dim, dim * mult), nn.GELU(), nn.Identity(), nn.Linear(dim * mult, dim)]

    def __call__(self, x):
        for layer in self.net:
            x = layer(x)
        return x


class Transformer(nn.Module):
    def __init__(self, dim: int, depth: int, heads: int, dim_head: int, norm_output: bool):
        super().__init__()
        self.layers = [[Attention(dim, heads, dim_head), FeedForward(dim)] for _ in range(depth)]
        self.norm = RMSNorm(dim) if norm_output else nn.Identity()

    def __call__(self, x):
        for attn, ff in self.layers:
            x = attn(x) + x
            x = ff(x) + x
        return self.norm(x)


class BandSplit(nn.Module):
    def __init__(self, dim: int, dim_inputs: tuple[int, ...]):
        super().__init__()
        self.dim_inputs = dim_inputs
        self.to_features = [[RMSNorm(d), nn.Linear(d, dim)] for d in dim_inputs]

    def __call__(self, x):
        splits = np.cumsum(self.dim_inputs)[:-1].tolist()
        parts = mx.split(x, splits, axis=-1)
        return mx.stack([lin(norm(p)) for p, (norm, lin) in zip(parts, self.to_features)], axis=-2)


class MaskEstimator(nn.Module):
    def __init__(self, dim: int, dim_inputs: tuple[int, ...], hidden_layers: int, expansion: int):
        super().__init__()
        hidden = dim * expansion
        self.to_freqs = []
        for d in dim_inputs:
            dims = (dim, *(hidden,) * hidden_layers, d * 2)
            mlp = []
            for i, (a, b) in enumerate(zip(dims[:-1], dims[1:])):
                mlp.append(nn.Linear(a, b))
                if i < len(dims) - 2:
                    mlp.append(nn.Tanh())
            self.to_freqs.append([mlp, nn.GLU(axis=-1)])

    def __call__(self, x):
        outs = []
        for band in range(x.shape[-2]):
            mlp, glu = self.to_freqs[band]
            h = x[..., band, :]
            for layer in mlp:
                h = layer(h)
            outs.append(glu(h))
        return mx.concatenate(outs, axis=-1)


class Roformer(nn.Module):
    """Mel-Band RoFormer, or BS-RoFormer when ``cfg.freqs_per_bands`` is set."""

    def __init__(self, cfg: ModelConfig):
        super().__init__()
        self.cfg = cfg
        self.channels = 2 if cfg.stereo else 1
        freqs = cfg.stft_n_fft // 2 + 1

        # Mel-RoFormer normalizes inside every transformer; BS-RoFormer once at the end.
        self.layers = [
            [
                Transformer(cfg.dim, cfg.time_transformer_depth, cfg.heads, cfg.dim_head, norm_output=cfg.is_mel),
                Transformer(cfg.dim, cfg.freq_transformer_depth, cfg.heads, cfg.dim_head, norm_output=cfg.is_mel),
            ]
            for _ in range(cfg.depth)
        ]
        if not cfg.is_mel:
            self.final_norm = RMSNorm(cfg.dim)

        if cfg.is_mel:
            bands = mel_band_mask(cfg.sample_rate, cfg.stft_n_fft, cfg.num_bands)
            freq_indices = np.nonzero(bands)[1]  # band-major, ascending within each band
            if cfg.stereo:
                freq_indices = (freq_indices[:, None] * 2 + np.arange(2)).reshape(-1)
            self._freq_indices = mx.array(freq_indices.astype(np.int32))
            self._bands_per_freq = mx.array(bands.sum(axis=0).astype(np.float32))
            band_sizes = bands.sum(axis=1)
        else:
            assert sum(cfg.freqs_per_bands) == freqs, f"bands cover {sum(cfg.freqs_per_bands)} bins, STFT has {freqs}"
            band_sizes = cfg.freqs_per_bands
        self._window = mx.array(hann_window(cfg.stft_win_length))

        dim_inputs = tuple(int(n) * 2 * self.channels for n in band_sizes)
        self.band_split = BandSplit(cfg.dim, dim_inputs)
        # the reference MLP() builds `depth` hidden layers for Mel-RoFormer, `depth - 1` for BS-RoFormer
        hidden_layers = cfg.mask_estimator_depth if cfg.is_mel else cfg.mask_estimator_depth - 1
        self.mask_estimators = [
            MaskEstimator(cfg.dim, dim_inputs, hidden_layers, cfg.mlp_expansion_factor)
            for _ in range(cfg.num_stems)
        ]

    def __call__(self, audio: mx.array) -> mx.array:
        """(B, channels, T) float32 audio -> (B, stems, channels, T) separated audio."""
        cfg = self.cfg
        b, c, t = audio.shape
        n_fft, hop = cfg.stft_n_fft, cfg.stft_hop_length

        spec = stft(audio.reshape(b * c, t), n_fft, hop, self._window)  # (b*c, frames, F)
        frames, freqs = spec.shape[1], spec.shape[2]
        # (b, frames, F*c) with stereo interleaved per bin, matching torch's '(f s)'
        spec = spec.reshape(b, c, frames, freqs).transpose(0, 2, 3, 1).reshape(b, frames, freqs * c)

        x = mx.take(spec, self._freq_indices, axis=-1) if cfg.is_mel else spec
        x = mx.stack([mx.real(x), mx.imag(x)], axis=-1).reshape(b, frames, -1)
        x = x.astype(self.band_split.to_features[0][1].weight.dtype)

        x = self.band_split(x)  # (b, frames, bands, dim)
        bands, dim = x.shape[2], x.shape[3]
        for time_tf, freq_tf in self.layers:
            x = x.transpose(0, 2, 1, 3).reshape(b * bands, frames, dim)
            x = time_tf(x)
            x = x.reshape(b, bands, frames, dim).transpose(0, 2, 1, 3).reshape(b * frames, bands, dim)
            x = freq_tf(x)
            x = x.reshape(b, frames, bands, dim)
        if not cfg.is_mel:
            x = self.final_norm(x)

        outputs = []
        for estimator in self.mask_estimators:
            m = estimator(x).astype(mx.float32).reshape(b, frames, -1, 2)
            mask = m[..., 0] + 1j * m[..., 1]
            if cfg.is_mel:
                # bins shared by several mel bands get the average of their masks
                summed = mx.zeros(spec.shape, dtype=mx.complex64).at[:, :, self._freq_indices].add(mask)
                mask = summed / mx.repeat(self._bands_per_freq, c)
            out = spec * mask
            out = out.reshape(b, frames, freqs, c).transpose(0, 3, 1, 2)  # (b, c, frames, F)
            if cfg.zero_dc:
                out = mx.concatenate([mx.zeros_like(out[..., :1]), out[..., 1:]], axis=-1)
            wav = istft(out.reshape(b * c, frames, freqs), n_fft, hop, self._window, length=t)
            outputs.append(wav.reshape(b, c, t))
        return mx.stack(outputs, axis=1)

