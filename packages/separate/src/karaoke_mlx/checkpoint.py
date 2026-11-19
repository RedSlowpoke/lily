"""Read PyTorch ``.ckpt`` state dicts without importing torch.

A torch zip checkpoint is a pickle (``data.pkl``) whose tensors point at raw
storage blobs (``data/<key>``). The unpickler below only resolves the handful
of globals a plain state dict needs and refuses everything else, so loading a
downloaded checkpoint cannot execute arbitrary code.
"""

from __future__ import annotations

import pickle
import zipfile
from collections import OrderedDict
from pathlib import Path

import numpy as np

_STORAGE_DTYPES = {
    "FloatStorage": np.float32,
    "HalfStorage": np.float16,
    "DoubleStorage": np.float64,
    "LongStorage": np.int64,
    "IntStorage": np.int32,
    "ShortStorage": np.int16,
    "CharStorage": np.int8,
    "ByteStorage": np.uint8,
    "BoolStorage": np.bool_,
    # numpy has no bfloat16; these are widened to float32 in _rebuild_tensor.
    "BFloat16Storage": "bfloat16",
}


class _StorageType:
    def __init__(self, name: str):
        self.dtype = _STORAGE_DTYPES[name]


def _rebuild_tensor(storage, offset, size, stride, *_):
    data, itemsize = storage
    if not size:
        return data[offset : offset + 1].reshape(())
    view = np.lib.stride_tricks.as_strided(
        data[offset:], shape=tuple(size), strides=tuple(s * itemsize for s in stride)
    )
    return np.ascontiguousarray(view)


def _rebuild_parameter(tensor, *_):
    return tensor


class _Unpickler(pickle.Unpickler):
    def __init__(self, file, archive: zipfile.ZipFile, prefix: str):
        super().__init__(file)
        self._archive = archive
        self._prefix = prefix

    def find_class(self, module, name):
        if module == "collections" and name == "OrderedDict":
            return OrderedDict
        if module == "torch._utils" and name == "_rebuild_tensor_v2":
            return _rebuild_tensor
        if module == "torch._utils" and name == "_rebuild_parameter":
            return _rebuild_parameter
        if module == "torch" and name in _STORAGE_DTYPES:
            return _StorageType(name)
        raise pickle.UnpicklingError(f"refusing to load {module}.{name} from checkpoint")

    def persistent_load(self, pid):
        _, storage_type, key, _location, _numel = pid
        raw = self._archive.read(f"{self._prefix}data/{key}")
        if storage_type.dtype == "bfloat16":
            bits = np.frombuffer(raw, dtype=np.uint16).astype(np.uint32) << 16
            return bits.view(np.float32), 2
        data = np.frombuffer(raw, dtype=storage_type.dtype)
        return data, data.itemsize


def load_state_dict(path: str | Path) -> dict[str, np.ndarray]:
    """Return ``{name: ndarray}`` for a torch zip checkpoint."""
    with zipfile.ZipFile(path) as archive:
        pkl = next(n for n in archive.namelist() if n.endswith("data.pkl"))
        prefix = pkl[: -len("data.pkl")]
        with archive.open(pkl) as f:
            obj = _Unpickler(f, archive, prefix).load()

    for key in ("state_dict", "model", "model_state_dict"):
        if isinstance(obj, dict) and key in obj and isinstance(obj[key], dict):
            obj = obj[key]
    return {k.removeprefix("module."): v for k, v in obj.items() if isinstance(v, np.ndarray)}
