"""MLX port vs. the reference PyTorch MelBandRoformer / BSRoformer, using the real weights."""

import mlx.core as mx
import numpy as np
import pytest

torch = pytest.importorskip("torch")
librosa = pytest.importorskip("librosa")

from karaoke_mlx.model import ModelConfig, istft, mel_band_mask, stft, hann_window  # noqa: E402
from karaoke_mlx.models import MODELS, is_installed, load, weights_path  # noqa: E402


def test_mel_band_mask_matches_librosa():
    for sr, n_fft, bands in [(44100, 2048, 60), (44100, 2048, 64), (48000, 4096, 60)]:
        ref = librosa.filters.mel(sr=sr, n_fft=n_fft, n_mels=bands)
        ref[0][0] = 1.0
        ref[-1, -1] = 1.0
        np.testing.assert_array_equal(mel_band_mask(sr, n_fft, bands), ref > 0)


def test_stft_roundtrip_matches_torch():
    rng = np.random.default_rng(0)
    x = rng.standard_normal((2, 44100 * 2)).astype(np.float32)
    window = hann_window(2048)
    ours = np.array(stft(mx.array(x), 2048, 441, mx.array(window)))
    ref = torch.stft(torch.from_numpy(x), 2048, 441, window=torch.from_numpy(window), return_complex=True)
    np.testing.assert_allclose(ours, ref.transpose(1, 2).numpy(), atol=2e-3, rtol=1e-4)

    back = np.array(istft(mx.array(ours), 2048, 441, mx.array(window)))
    np.testing.assert_allclose(back, x[:, : back.shape[1]], atol=1e-4)


@pytest.mark.parametrize("model_id", list(MODELS))
def test_model_matches_reference(model_id):
    spec = MODELS[model_id]
    if not is_installed(spec):
        pytest.skip(f"{model_id} weights not installed (run `karaoke-mlx download {model_id}`)")

    cfg: ModelConfig = spec.config
    common = dict(
        dim=cfg.dim, depth=cfg.depth, stereo=cfg.stereo, num_stems=cfg.num_stems,
        time_transformer_depth=cfg.time_transformer_depth, freq_transformer_depth=cfg.freq_transformer_depth,
        dim_head=cfg.dim_head, heads=cfg.heads, attn_dropout=0, ff_dropout=0,
        stft_n_fft=cfg.stft_n_fft, stft_hop_length=cfg.stft_hop_length,
        stft_win_length=cfg.stft_win_length, mask_estimator_depth=cfg.mask_estimator_depth,
    )
    if cfg.is_mel:
        from tests.reference.mel_band_roformer import MelBandRoformer

        ref = MelBandRoformer(num_bands=cfg.num_bands, sample_rate=cfg.sample_rate, **common).eval()
    else:
        from tests.reference.bs_roformer import BSRoformer

        ref = BSRoformer(freqs_per_bands=cfg.freqs_per_bands, **common).eval()
    weights = {k: torch.from_numpy(np.array(v)) for k, v in mx.load(str(weights_path(spec))).items()}
    missing, unexpected = ref.load_state_dict(weights, strict=False)
    assert not unexpected and all("rotary_embed" in k for k in missing)

    # 3 s of a chord plus noise
    t = np.arange(44100 * 3) / 44100
    rng = np.random.default_rng(1)
    tone = sum(np.sin(2 * np.pi * f * t) for f in (220, 277.2, 329.6, 440)) * 0.15
    x = np.stack([tone, np.roll(tone, 50)]) + 0.02 * rng.standard_normal((2, t.size))
    x = x[None].astype(np.float32)

    with torch.inference_mode():
        expected = ref(torch.from_numpy(x)).numpy()
    if expected.ndim == 3:  # single-stem models drop the stem axis
        expected = expected[:, None]

    ours = np.array(load(spec, dtype=mx.float32)(mx.array(x)))
    assert ours.shape == expected.shape
    err = np.abs(ours - expected).max() / np.abs(expected).max()
    print(f"{model_id}: fp32 max rel err {err:.2e}", end=" ")
    assert err < 1e-3, f"max relative error {err:.2e}"

    half = np.array(load(spec, dtype=mx.float16)(mx.array(x)).astype(mx.float32))
    snr = 10 * np.log10(np.sum(expected**2) / np.sum((half - expected) ** 2))
    print(f"fp16 SNR {snr:.1f} dB")
    assert snr > 30, f"float16 SNR {snr:.1f} dB"

