"""``karaoke-mlx`` command line.

With ``--json`` every line on stdout is one JSON event, which is how
Lily Studio drives it (DECISIONS D112):

    {"event": "status", "message": "..."}
    {"event": "progress", "stage": "download|verify|convert|load|decode|separate|mix|write", "value": 0.0-1.0}
    {"event": "done", "outputs": [{"stem": "vocals", "path": "/path"}, ...], "folder": "/path", "seconds": 12.3}
    {"event": "error", "message": "..."}
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

import mlx.core as mx
import numpy as np

from . import audio
from .models import DEFAULT_MODEL, MODELS, install, is_installed, load
from .separate import separate

DTYPES = {"float16": mx.float16, "bfloat16": mx.bfloat16, "float32": mx.float32}


class Reporter:
    def __init__(self, as_json: bool):
        self.as_json = as_json
        self._last: tuple[str, int] | None = None

    def emit(self, event: str, **data) -> None:
        if self.as_json:
            print(json.dumps({"event": event, **data}), flush=True)

    def status(self, message: str) -> None:
        if self.as_json:
            self.emit("status", message=message)
        else:
            print(message, file=sys.stderr, flush=True)

    def progress(self, stage: str, value: float) -> None:
        key = (stage, int(value * 100))
        if key == self._last:
            return
        self._last = key
        if self.as_json:
            self.emit("progress", stage=stage, value=round(value, 4))
        else:
            print(f"\r{stage}: {value:6.1%}", end="\n" if value >= 1 else "", file=sys.stderr, flush=True)


def cmd_models(args, out: Reporter) -> None:
    rows = [
        {
            "id": spec.id,
            "title": spec.title,
            "description": spec.description,
            "task": spec.task,
            "stems": spec.outputs,
            "size": spec.size,
            "installed": is_installed(spec),
            "default": spec.id == DEFAULT_MODEL,
            "num_overlap": spec.num_overlap,
        }
        for spec in MODELS.values()
    ]
    if args.json:
        print(json.dumps(rows), flush=True)
        return
    for r in rows:
        flag = "installed" if r["installed"] else f"{r['size'] / 1e9:.1f} GB download"
        print(f"{r['id']:<15} {r['title']}  [{flag}]{'  (default)' if r['default'] else ''}")
        print(f"{'':<15} -> {', '.join(r['stems'])}")


def cmd_download(args, out: Reporter) -> None:
    spec = MODELS[args.model]
    path = install(spec, out.progress)
    out.status(f"installed {spec.id} -> {path}")
    out.emit("done", outputs=[{"stem": "weights", "path": str(path)}])


def cmd_separate(args, out: Reporter) -> None:
    started = time.time()
    spec = MODELS[args.model]
    src = Path(args.input).expanduser().resolve()
    suffix = "karaoke" if spec.task == "karaoke" else "stems"
    dest = Path(args.output).expanduser() if args.output else src.parent / f"{src.stem} ({suffix})"
    dest.mkdir(parents=True, exist_ok=True)

    if not is_installed(spec):
        out.status(f"downloading {spec.title} ({spec.size / 1e9:.1f} GB, one time)")
    out.progress("load", 0.0)
    model = load(spec, DTYPES[args.dtype], out.progress)
    out.progress("load", 1.0)

    out.status(f"decoding {src.name}")
    out.progress("decode", 0.0)
    mix = audio.load(src, spec.config.sample_rate)
    out.progress("decode", 1.0)

    out.status(f"separating with {spec.title}")
    stems = separate(
        model, spec, mix,
        num_overlap=args.overlap,
        batch_size=args.batch_size,
        progress=lambda v: out.progress("separate", v),
    )

    out.progress("write", 0.0)
    outputs = []
    for i, (name, data) in enumerate(stems.items()):
        path = dest / f"{src.stem} - {name}.{args.format}"
        audio.save(path, data, spec.config.sample_rate)
        outputs.append({"stem": name, "path": str(path)})
        out.progress("write", (i + 1) / len(stems))

    seconds = time.time() - started
    out.status(f"done in {seconds:.1f}s -> {dest}")
    out.emit("done", outputs=outputs, folder=str(dest), seconds=round(seconds, 2))


def cmd_mix(args, out: Reporter) -> None:
    """Sum several stems back into one track (stems of a song add up to the mix)."""
    started = time.time()
    dest = Path(args.output).expanduser()
    if dest.suffix.lstrip(".").lower() not in audio.ENCODERS:
        raise ValueError(f"output must end in one of: {', '.join('.' + f for f in audio.ENCODERS)}")
    tracks = []
    for i, path in enumerate(args.stems):
        tracks.append(audio.load(Path(path).expanduser()))
        out.progress("mix", (i + 1) / len(args.stems))
    length = max(t.shape[-1] for t in tracks)
    mix = sum(np.pad(t, ((0, 0), (0, length - t.shape[-1]))) for t in tracks)

    out.progress("write", 0.0)
    dest.parent.mkdir(parents=True, exist_ok=True)
    audio.save(dest, mix)
    out.progress("write", 1.0)
    seconds = time.time() - started
    out.status(f"mixed {len(tracks)} stems in {seconds:.1f}s -> {dest}")
    out.emit("done", outputs=[{"stem": "mix", "path": str(dest)}], folder=str(dest.parent), seconds=round(seconds, 2))


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(prog="karaoke-mlx", description=__doc__.splitlines()[0])
    parser.add_argument("--json", action="store_true", help="emit JSON lines on stdout")
    sub = parser.add_subparsers(dest="command", required=True)

    sub.add_parser("models", help="list available models")

    p = sub.add_parser("download", help="download and convert a model")
    p.add_argument("model", choices=MODELS, nargs="?", default=DEFAULT_MODEL)

    p = sub.add_parser("separate", help="split a song into stems (karaoke: instrumental + lead vocals)")
    p.add_argument("input")
    p.add_argument("-o", "--output", help="output folder (default: '<song> (karaoke|stems)' next to the input)")
    p.add_argument("-m", "--model", choices=MODELS, default=DEFAULT_MODEL)
    p.add_argument("--overlap", type=int, help="chunks overlapping each sample; higher = smoother but slower (default: model's)")
    p.add_argument("--batch-size", type=int, default=1)
    p.add_argument("--dtype", choices=DTYPES, default="float16")
    p.add_argument("-f", "--format", choices=audio.ENCODERS, default="wav")

    p = sub.add_parser("mix", help="combine stems into one audio file")
    p.add_argument("stems", nargs="+", help="stem files to sum")
    p.add_argument("-o", "--output", required=True, help="output file (.wav, .flac or .mp3)")

    args = parser.parse_args(argv)
    out = Reporter(args.json)
    handler = {"models": cmd_models, "download": cmd_download, "separate": cmd_separate, "mix": cmd_mix}[args.command]
    try:
        handler(args, out)
    except KeyboardInterrupt:
        sys.exit(130)
    except Exception as exc:  # report, then fail
        if args.json:
            out.emit("error", message=str(exc))
            sys.exit(1)
        raise


if __name__ == "__main__":
    main()
