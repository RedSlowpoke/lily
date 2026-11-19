"""Known RoFormer checkpoints, plus download / conversion / loading."""

from __future__ import annotations

import dataclasses
import hashlib
import json
import os
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable

import mlx.core as mx
from mlx.utils import tree_flatten

from .checkpoint import load_state_dict
from .model import ModelConfig, Roformer


@dataclass(frozen=True)
class ModelSpec:
    id: str
    title: str
    description: str
    url: str
    sha256: str
    size: int
    # "karaoke" or "stems"; groups models in the UI
    task: str
    # names of the stems the network outputs, in order
    stems: tuple[str, ...]
    chunk_size: int
    num_overlap: int
    config: ModelConfig = field(default_factory=ModelConfig)
    # extra stem computed as mix minus everything the network outputs
    residual: str | None = None
    # order to present / write stems in; defaults to stems then residual
    order: tuple[str, ...] | None = None

    @property
    def outputs(self) -> tuple[str, ...]:
        return self.order or (*self.stems, *((self.residual,) if self.residual else ()))


MODELS: dict[str, ModelSpec] = {
    spec.id: spec
    for spec in (
        ModelSpec(
            id="becruily",
            title="Mel-RoFormer Karaoke (becruily)",
            description="Two-stem karaoke model: lead vocals vs. instrumental with backing vocals kept.",
            url="https://huggingface.co/becruily/mel-band-roformer-karaoke/resolve/main/mel_band_roformer_karaoke_becruily.ckpt",
            sha256="d3aa262ac01df870b9fc033e9c7b6cad33fe04fc9c148b6c40841326a515a0e0",
            size=1_719_139_254,
            task="karaoke",
            stems=("vocals", "instrumental"),
            order=("instrumental", "vocals"),
            chunk_size=485_100,
            num_overlap=8,
            config=ModelConfig(num_stems=2),
        ),
        ModelSpec(
            id="aufr33-viperx",
            title="Mel-RoFormer Karaoke (aufr33 & viperx)",
            description="The original Mel-RoFormer karaoke model; predicts lead vocals, instrumental = mix - lead.",
            url="https://github.com/TRvlvr/model_repo/releases/download/all_public_uvr_models/mel_band_roformer_karaoke_aufr33_viperx_sdr_10.1956.ckpt",
            sha256="1de20d459332fe8869aeb01327a31df0032262706e1365114e852dc271779813",
            size=913_096_801,
            task="karaoke",
            stems=("vocals",),
            residual="instrumental",
            order=("instrumental", "vocals"),
            chunk_size=352_800,
            num_overlap=4,
            config=ModelConfig(num_stems=1),
        ),
        ModelSpec(
            id="bs-roformer-sw",
            title="BS-RoFormer SW, 6 stems (jarredou)",
            description="Splits a song into vocals, drums, bass, guitar, piano and other.",
            url="https://github.com/nomadkaraoke/python-audio-separator/releases/download/model-configs/BS-Roformer-SW.ckpt",
            sha256="24e7d35ee9c64415673d3fd33e06a67cac2c103c5df6267ba1576459c775916e",
            size=699_412_152,
            task="stems",
            stems=("bass", "drums", "other", "vocals", "guitar", "piano"),
            order=("vocals", "drums", "bass", "guitar", "piano", "other"),
            chunk_size=588_800,
            num_overlap=2,
            config=ModelConfig(
                dim=256,
                depth=12,
                num_stems=6,
                stft_hop_length=512,
                freqs_per_bands=(*(2,) * 24, *(4,) * 12, *(12,) * 8, *(24,) * 8, *(48,) * 8, 128, 129),
            ),
        ),
    )
}

DEFAULT_MODEL = "becruily"

Progress = Callable[[str, float], None]


def home() -> Path:
    return Path(os.environ.get("KARAOKE_MLX_HOME", Path.home() / ".cache" / "karaoke-mlx"))


def weights_path(spec: ModelSpec) -> Path:
    return home() / "models" / spec.id / "model.safetensors"


def is_installed(spec: ModelSpec) -> bool:
    return weights_path(spec).exists()


def _download(spec: ModelSpec, dest: Path, progress: Progress | None) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.exists() and dest.stat().st_size == spec.size:
        return
    part = dest.with_suffix(dest.suffix + ".part")
    done = part.stat().st_size if part.exists() else 0
    request = urllib.request.Request(spec.url, headers={"Range": f"bytes={done}-"} if done else {})
    with urllib.request.urlopen(request) as response:
        if done and response.status != 206:
            done = 0  # server ignored the range; start over
        with open(part, "ab" if done else "wb") as f:
            while chunk := response.read(1 << 20):
                f.write(chunk)
                done += len(chunk)
                if progress:
                    progress("download", done / spec.size)
    part.rename(dest)


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(1 << 24):
            h.update(chunk)
    return h.hexdigest()


def convert(spec: ModelSpec, ckpt: Path) -> Path:
    """Convert a torch checkpoint into MLX safetensors, validating every tensor shape."""
    state = {k: v for k, v in load_state_dict(ckpt).items() if "rotary_embed" not in k}
    model = Roformer(spec.config)
    expected = dict(tree_flatten(model.parameters()))
    missing = expected.keys() - state.keys()
    unexpected = state.keys() - expected.keys()
    if missing or unexpected:
        raise ValueError(f"checkpoint does not match model: missing={sorted(missing)[:5]} unexpected={sorted(unexpected)[:5]}")
    for k, v in expected.items():
        if tuple(v.shape) != state[k].shape:
            raise ValueError(f"{k}: checkpoint shape {state[k].shape} != model shape {tuple(v.shape)}")

    out = weights_path(spec)
    out.parent.mkdir(parents=True, exist_ok=True)
    tmp = out.with_name("model.tmp.safetensors")
    mx.save_safetensors(str(tmp), {k: mx.array(v) for k, v in state.items()})
    tmp.rename(out)
    (out.parent / "spec.json").write_text(json.dumps(dataclasses.asdict(spec), indent=2))
    return out


def install(spec: ModelSpec, progress: Progress | None = None, keep_checkpoint: bool = False) -> Path:
    if is_installed(spec):
        return weights_path(spec)
    ckpt = home() / "downloads" / Path(spec.url).name
    _download(spec, ckpt, progress)
    if progress:
        progress("verify", 0.0)
    digest = _sha256(ckpt)
    if digest != spec.sha256:
        ckpt.unlink()
        raise ValueError(f"checksum mismatch for {ckpt.name}: got {digest}")
    if progress:
        progress("convert", 0.0)
    path = convert(spec, ckpt)
    if not keep_checkpoint:
        ckpt.unlink()
    return path


def load(spec: ModelSpec, dtype: mx.Dtype = mx.float16, progress: Progress | None = None) -> Roformer:
    path = install(spec, progress)
    model = Roformer(spec.config)
    weights = mx.load(str(path))
    model.load_weights([(k, v.astype(dtype)) for k, v in weights.items()], strict=True)
    model.eval()
    mx.eval(model.parameters())
    return model

