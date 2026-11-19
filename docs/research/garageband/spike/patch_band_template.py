#!/usr/bin/env python3
"""Patch a .band that GarageBand saved, and see whether GarageBand opens it (§5).

    python3 -I patch_band_template.py TEMPLATE.band OUT_DIR [VARIANT ...]
    python3 -I patch_band_template.py --open OUT_DIR [VARIANT ...]

The first form copies TEMPLATE.band once per variant into OUT_DIR/<variant>.band
and changes, in each copy, only what §2.6 found readable:

    copy         nothing: the package copied byte for byte to a new place
    tempo        every tempo event scaled so the first is 100 bpm, with the
                 Song record's two tempos and MetaData's BeatsPerMinute
    tempo-meta   MetaData's BeatsPerMinute only, to see which file wins
    rename-same  one region renamed to a name of the same length
    rename       regions and channel strips renamed, longer and shorter
    media-same   one audio file's samples swapped for others of its length
    media        one audio file swapped for a new file of another name and
                 length: AuFl, AuRg, the strip's name, MetaData, Media/
    all          tempo, rename and media together
    stale-path   media-same, with the file's folder in AuFl set to a path
                 that does not exist
    other-path   media-same, with that folder set to the template's own
                 Media/Audio Files, which holds the old samples
    bad-length   a control: the file header's length 16 bytes too long
    bad-record   a control: one record's length one byte too long, so the
                 walk loses its place after it
    bad-name     a control: a region's name length 4 bytes too long, inside
                 a record whose own length is right

The names to change are those of the template §5.1 used (step 4's stems.band,
whose tracks are Violin, Cello, ... and whose audio files are Song.wav,
Drums.wav and Bass.wav); a variant whose names are not in the template stops
with an error. Records are changed in place or resized, never added or
removed, and every record length and the file header's length are rewritten.

The second form opens each OUT_DIR/<variant>.band in GarageBand through
AppleScript, has GarageBand save it to OUT_DIR/resaved/<variant>.band, closes
it, and reads back what GarageBand kept. It needs GarageBand 10.4.14 running
and the terminal's Apple Events consent (§3.6). It first closes any open
project that has no unsaved changes, and stops if one has. The bad- controls
are opened only when named: bad-record crashes GarageBand (§5.4).

Standard library only; it imports inspect_band.py from beside it.
"""

from __future__ import annotations

import math
import os
import plistlib
import shutil
import struct
import subprocess
import sys
import time
import wave

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # -I leaves it out
import inspect_band as ib  # noqa: E402

TEMPO_KIND = 3  # EvSq class: the tempo map (§2.6)
NEW_BPM = 100.0
MSEQ_NAME = 0x10  # u16 length, then UTF-8 (§2.6)
ENVI_NAME = 0x9E
AURG_FRAMES, AURG_NAME = 0x16, 0x4A
# AuFl: u32, u32, u16 characters, UTF-16LE name; then, from the name's end:
AUFL_NAME = 0x08
AUFL_PATH, AUFL_PATH_LEN = 0x8A, 0x100  # the folder, absolute or relative, NUL-padded
AUFL_SIZE = 0x196  # the file's size in bytes
AUFL_WAVE = 0x1C8  # 'EVAW', u32 0, u32 data offset, u32 frames, u32 0, u32 rate, u16 ch, u16 bits
AUFL_LGWV = 0x212  # the size of GarageBand's LGWV chunk at the file's end

# What the variants rename in the template (§5.1).
REGIONS = {"same": [("Violin", "Fiddle")],
           "longer": [("Violin", "Violin I (solo)"), ("Cello", "Violoncello"), ("Drums", "Drum kit")]}
STRIPS = [(44, "String Ensemble", "Violin I (solo)"), (45, "String Ensemble", "Violoncello")]
SWAP_FROM = "Song.wav"
SWAP_TO = "Voice take 2.wav"
SWAP_SECONDS = 9.0


class Project:
    """A copy of a .band in memory: its ProjectData as records, and its MetaData."""

    def __init__(self, band: str):
        self.band = band
        self.alt = ib.alternatives(band)[0]
        data = open(os.path.join(self.alt, "ProjectData"), "rb").read()
        chunks, end = ib.walk(data)
        if end != len(data):
            raise SystemExit("%s: ProjectData does not walk to its end" % band)
        self.head = bytearray(data[:ib.FILE_HEADER])
        self.records = [[bytearray(data[c.offset:c.offset + ib.CHUNK_HEADER]), bytearray(c.body)]
                        for c in chunks]
        self.meta = plistlib.load(open(os.path.join(self.alt, "MetaData.plist"), "rb"))
        self.media: dict[str, str | None] = {}  # Media/Audio Files/<name> -> source path, or None to remove
        self.log: list[str] = []
        self.length_error = 0  # added to the file header's length, for the `bad-length` control
        self.record_error: tuple[int, int] | None = None  # (record number, bytes) for `bad-record`

    def tagged(self, tag: str) -> list[tuple[bytearray, bytearray]]:
        return [(h, b) for h, b in self.records if h[:4][::-1].decode("latin-1") == tag]

    @staticmethod
    def kind(h: bytearray) -> int:
        return struct.unpack_from("<H", h, 6)[0]

    @staticmethod
    def index(h: bytearray) -> int:
        return struct.unpack_from("<I", h, 8)[0] >> 18

    # -- what §2.6 found readable --------------------------------------------

    def scale_tempo(self, ratio: float, data: bool = True, meta: bool = True) -> None:
        if data:
            n = 0
            for h, b in self.tagged("EvSq"):
                if self.kind(h) != TEMPO_KIND:
                    continue
                for i in range(0, len(b) - 2 * ib.EVENT + 1, ib.EVENT):
                    if b[i] == 0x60 and not b[i + 7] & 0x80 and b[i + ib.EVENT + 7] == 0x88:
                        old = struct.unpack_from("<I", b, i + ib.EVENT)[0]
                        struct.pack_into("<I", b, i + ib.EVENT, round(old * ratio))
                        self.log.append("tempo %.2f -> %.2f" % (old / 1e4, round(old * ratio) / 1e4))
                        n += 1
            if not n:
                raise SystemExit("no tempo events in the template")
            (_, song), = self.tagged("Song")
            for off in (110, 114):
                old = struct.unpack_from("<I", song, off)[0]
                struct.pack_into("<I", song, off, round(old * ratio))
        if meta:
            old = self.meta["BeatsPerMinute"]
            self.meta["BeatsPerMinute"] = round(old * ratio, 2)
            self.log.append("MetaData BeatsPerMinute %.2f -> %.2f" % (old, self.meta["BeatsPerMinute"]))

    def rename_region(self, old: str, new: str) -> None:
        hits = [b for _, b in self.tagged("MSeq") if name_at(b, MSEQ_NAME) == old]
        if not hits:
            raise SystemExit("no sequence named %r in the template" % old)
        for b in hits:
            set_name(b, MSEQ_NAME, new)
        self.log.append("region %r -> %r (%d MSeq)" % (old, new, len(hits)))

    def rename_strip(self, index: int, old: str, new: str) -> None:
        hits = [b for h, b in self.tagged("Envi") if self.index(h) == index and name_at(b, ENVI_NAME) == old]
        if len(hits) != 1:
            raise SystemExit("no channel strip %d named %r in the template" % (index, old))
        set_name(hits[0], ENVI_NAME, new)
        self.log.append("strip %d %r -> %r" % (index, old, new))

    def swap_audio(self, old: str, wav: str, new: str | None = None) -> None:
        """Put `wav` in place of the audio file `old`, under the name `new` if given."""
        new = new or old
        with wave.open(wav, "rb") as w:
            frames, rate, channels, bits = w.getnframes(), w.getframerate(), w.getnchannels(), 8 * w.getsampwidth()
        size = os.path.getsize(wav)
        files = [b for _, b in self.tagged("AuFl") if aufl_name(b) == old]
        if len(files) != 1:
            raise SystemExit("no audio file %r in the template" % old)
        b = files[0]
        end = AUFL_NAME + 2 + 2 * len(old)
        if b[end:end + 4] != b"LFUA" or b[end + AUFL_WAVE:end + AUFL_WAVE + 4] != b"EVAW":
            raise SystemExit("AuFl of %r is not laid out as §5 expects" % old)
        w = end + AUFL_WAVE
        struct.pack_into("<I", b, end + AUFL_SIZE, size)
        struct.pack_into("<III", b, w + 8, 44, frames, 0)
        struct.pack_into("<IHH", b, w + 0x14, rate, channels, bits)
        struct.pack_into("<I", b, end + AUFL_LGWV, 0)  # no LGWV: does GarageBand write one?
        name16 = new.encode("utf-16-le")
        b[AUFL_NAME:end] = struct.pack("<H", len(new)) + name16
        old_stem, new_stem = os.path.splitext(old)[0], os.path.splitext(new)[0]
        regions = [rb for _, rb in self.tagged("AuRg") if name_at(rb, AURG_NAME) == old_stem]
        for rb in regions:
            struct.pack_into("<I", rb, AURG_FRAMES, frames)
            set_name(rb, AURG_NAME, new_stem)
        strips = [eb for _, eb in self.tagged("Envi") if name_at(eb, ENVI_NAME) == old_stem]
        for eb in strips:
            set_name(eb, ENVI_NAME, new_stem)
        self.meta["AudioFiles"] = [p.replace(old, new) if os.path.basename(p) == old else p
                                   for p in self.meta["AudioFiles"]]
        self.media[old] = None
        self.media[new] = wav
        self.log.append("audio %r -> %r: %d frames, %d bytes; %d AuRg, %d Envi renamed"
                        % (old, new, frames, size, len(regions), len(strips)))

    def set_folder(self, name: str, folder: str) -> None:
        """Point the audio file `name` at another folder (AuFl keeps one, absolute or relative to Media/)."""
        files = [b for _, b in self.tagged("AuFl") if aufl_name(b) == name]
        if len(files) != 1:
            raise SystemExit("no audio file %r in the template" % name)
        end = AUFL_NAME + 2 + 2 * len(name)
        raw = folder.encode("utf-8")
        if len(raw) >= AUFL_PATH_LEN:
            raise SystemExit("folder too long for AuFl: %s" % folder)
        files[0][end + AUFL_PATH:end + AUFL_PATH + AUFL_PATH_LEN] = raw.ljust(AUFL_PATH_LEN, b"\0")
        self.log.append("folder of %r -> %r" % (name, folder))

    # -- writing -------------------------------------------------------------

    def write(self, dest: str) -> None:
        if os.path.exists(dest):
            shutil.rmtree(dest)
        shutil.copytree(self.band, dest, symlinks=True)
        body = bytearray()
        for i, (h, b) in enumerate(self.records):
            err = self.record_error[1] if self.record_error and self.record_error[0] == i else 0
            struct.pack_into("<Q", h, 28, len(b) + err)
            body += h + b
        struct.pack_into("<Q", self.head, 16, len(body) + self.length_error)
        alt = os.path.join(dest, os.path.relpath(self.alt, self.band))
        with open(os.path.join(alt, "ProjectData"), "wb") as f:
            f.write(self.head + body)
        with open(os.path.join(alt, "MetaData.plist"), "wb") as f:
            plistlib.dump(self.meta, f, fmt=plistlib.FMT_XML)
        folder = os.path.join(dest, "Media", "Audio Files")
        for name, src in self.media.items():
            p = os.path.join(folder, name)
            if src is None:
                if os.path.exists(p):
                    os.remove(p)
            else:
                shutil.copyfile(src, p)


def name_at(b: bytearray, off: int) -> str | None:
    if off + 2 > len(b):
        return None
    (n,) = struct.unpack_from("<H", b, off)
    try:
        return bytes(b[off + 2:off + 2 + n]).decode("utf-8") if n else ""
    except UnicodeDecodeError:
        return None


def set_name(b: bytearray, off: int, new: str) -> None:
    (n,) = struct.unpack_from("<H", b, off)
    raw = new.encode("utf-8")
    b[off:off + 2 + n] = struct.pack("<H", len(raw)) + raw


def aufl_name(b: bytes) -> str:
    (n,) = struct.unpack_from("<H", b, AUFL_NAME)
    return bytes(b[AUFL_NAME + 2:AUFL_NAME + 2 + 2 * n]).decode("utf-16-le")


def make_wav(path: str, seconds: float, rate: int = 44100) -> None:
    """A tone that is plainly not the template's audio: an A every second, rising a fifth."""
    frames = round(seconds * rate)
    out = bytearray()
    for i in range(frames):
        t = i / rate
        beat, x = divmod(t, 1.0)
        f = 440.0 if int(beat) % 2 == 0 else 660.0
        v = 0.3 * math.sin(2 * math.pi * f * t) * math.exp(-4 * x)
        s = int(max(-1.0, min(1.0, v)) * 32767)
        out += struct.pack("<hh", s, s)
    with wave.open(path, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(bytes(out))


def wav_data(path: str) -> bytes:
    """A WAV's data chunk (GarageBand appends an LGWV chunk after it)."""
    data = open(path, "rb").read()
    i = 12
    while i + 8 <= len(data):
        tag, n = data[i:i + 4], struct.unpack_from("<I", data, i + 4)[0]
        if tag == b"data":
            return data[i + 8:i + 8 + n]
        i += 8 + n + (n & 1)
    raise ValueError("no data chunk in %s" % path)


def wav_frames(path: str) -> int:
    return len(wav_data(path)) // 4  # 16-bit stereo


VARIANTS = ["copy", "tempo", "tempo-meta", "rename-same", "rename", "media-same", "media", "all",
            "stale-path", "other-path", "bad-length", "bad-record", "bad-name"]


def build(template: str, out: str, which: list[str]) -> None:
    os.makedirs(out, exist_ok=True)
    src = os.path.join(template, "Media", "Audio Files", SWAP_FROM)
    same = os.path.join(out, "swap-same.wav")
    new = os.path.join(out, "swap-new.wav")
    if os.path.exists(src):
        make_wav(same, wav_frames(src) / 44100)
        make_wav(new, SWAP_SECONDS)
    ratio = None
    for v in which:
        p = Project(template)
        if ratio is None:
            first = next(ib.events(b) for h, b in p.tagged("EvSq") if p.kind(h) == TEMPO_KIND)
            bpm = float(next(e for e in first if e.startswith("tempo")).split()[2])
            ratio = NEW_BPM / bpm
        if v in ("tempo", "all"):
            p.scale_tempo(ratio)
        if v == "tempo-meta":
            p.scale_tempo(ratio, data=False)
        if v == "rename-same":
            for old, new_name in REGIONS["same"]:
                p.rename_region(old, new_name)
        if v in ("rename", "all"):
            for old, new_name in REGIONS["longer"]:
                p.rename_region(old, new_name)
            for index, old, new_name in STRIPS:
                p.rename_strip(index, old, new_name)
        if v in ("media-same", "stale-path", "other-path"):
            p.swap_audio(SWAP_FROM, same)
        if v == "stale-path":
            p.set_folder(SWAP_FROM, "/Users/Shared/No Such Project.band/Media/Audio Files")
        if v == "other-path":  # the template's own folder, which still holds the old samples
            p.set_folder(SWAP_FROM, os.path.join(os.path.abspath(template), "Media", "Audio Files"))
        if v in ("media", "all"):
            p.swap_audio(SWAP_FROM, new, SWAP_TO)
        if v == "bad-length":
            p.length_error = 16
            p.log.append("file header's length 16 too long")
        if v == "bad-record":
            i = next(i for i, (h, b) in enumerate(p.records) if h[:4] == b"qeSM" and name_at(b, MSEQ_NAME) == "Violin")
            p.record_error = (i, 1)
            p.log.append("record %d (MSeq Violin) length 1 too long" % i)
        if v == "bad-name":
            (h, b), = [(h, b) for h, b in p.tagged("MSeq") if name_at(b, MSEQ_NAME) == "Violin"]
            struct.pack_into("<H", b, MSEQ_NAME, len("Violin") + 4)
            p.log.append("MSeq Violin's name length 10 for 6 bytes")
        dest = os.path.join(out, v + ".band")
        p.write(dest)
        print("%-12s %s" % (v, "; ".join(p.log) or "unchanged"))


# -- GarageBand ------------------------------------------------------------------

def osa(script: str, timeout: float = 60) -> tuple[bool, str]:
    try:
        r = subprocess.run(["osascript", "-e", script], capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return False, "no answer in %ds (a dialog is probably up)" % timeout
    return r.returncode == 0, (r.stdout if r.returncode == 0 else r.stderr).strip()


def gb(script: str, timeout: float = 60) -> tuple[bool, str]:
    return osa('tell application "GarageBand" to ' + script, timeout)


def close_all() -> None:
    ok, n = gb("count of documents")
    if not ok:
        raise SystemExit("GarageBand does not answer: %s" % n)
    for _ in range(int(n)):
        ok, mod = gb("modified of document 1")
        if mod != "false":
            raise SystemExit("a modified project is open in GarageBand; save or close it first")
        gb("close document 1 saving no")


def open_and_resave(band: str, resaved: str) -> dict:
    close_all()
    r: dict = {}
    ok, msg = gb('open POSIX file "%s"' % band, timeout=90)
    r["open"] = "ok" if ok else msg
    for _ in range(60):
        ok, n = gb("count of documents", timeout=10)
        if ok and n not in ("", "0"):
            break
        time.sleep(1)
    else:
        r["documents"] = n
        return r
    time.sleep(3)  # let it settle before reading its state
    r["windows"] = gb("name of every window", timeout=10)[1]
    r["modified"] = gb("modified of document 1", timeout=10)[1]
    if os.path.exists(resaved):
        shutil.rmtree(resaved)
    ok, msg = gb('save document 1 in POSIX file "%s"' % resaved, timeout=90)
    r["save"] = "ok" if ok else msg
    gb("close document 1 saving no", timeout=30)
    return r


SYSTEM_SEQUENCES = {"Untitled", "TRASH", "Track Automation Root Folder", "Track Alternatives", "Global Harmonies"}
SYSTEM_STRIPS = {"Not Assigned", "MIDI Click", "(Folder)", "Sequencer Input", "Physical Input", "Input View",
                 "Input Notes", "Preview", "Click", "Bus 1", "Bus 2", "Stereo Out", "Master", "GM Device",
                 "@ (=Context Name)"}


def read_back(band: str) -> dict:
    p = Project(band)
    tempos = []
    for h, b in p.tagged("EvSq"):
        if p.kind(h) == TEMPO_KIND:
            tempos += [e.split()[2] for e in ib.events(b) if e.startswith("tempo")]
    regions = [n for n in (name_at(b, MSEQ_NAME) for _, b in p.tagged("MSeq"))
               if n and n not in SYSTEM_SEQUENCES and not n.startswith("*") and n != os.path.basename(band)[:-5]]
    strips = [n for n in (name_at(b, ENVI_NAME) for h, b in p.tagged("Envi") if p.kind(h) == 20)
              if n and n not in SYSTEM_STRIPS]
    audio = []
    for _, b in p.tagged("AuFl"):
        end = AUFL_NAME + 2 + 2 * len(aufl_name(b))
        w = end + AUFL_WAVE
        frames = struct.unpack_from("<I", b, w + 0x0C)[0]
        lgwv = struct.unpack_from("<I", b, end + AUFL_LGWV)[0]
        folder = bytes(b[end + AUFL_PATH:end + AUFL_PATH + AUFL_PATH_LEN]).split(b"\0")[0].decode()
        audio.append("%s %d frames, LGWV %d, folder %r" % (aufl_name(b), frames, lgwv, folder))
    media = sorted(os.listdir(os.path.join(band, "Media", "Audio Files"))) \
        if os.path.isdir(os.path.join(band, "Media", "Audio Files")) else []
    return {"tempos": tempos, "meta bpm": round(p.meta["BeatsPerMinute"], 2), "regions": regions,
            "strips": strips, "audio files": audio, "Media": media,
            "AudioFiles": [os.path.basename(a) for a in p.meta["AudioFiles"]]}


def check(out: str, which: list[str]) -> None:
    known = {wav_data(os.path.join(out, n)): label for n, label in
             (("swap-same.wav", "the swapped samples"), ("swap-new.wav", "the new file's samples"))
             if os.path.exists(os.path.join(out, n))}
    for v in which:
        band = os.path.abspath(os.path.join(out, v + ".band"))
        if not os.path.isdir(band):
            continue
        resaved = os.path.join(os.path.abspath(out), "resaved", v + ".band")
        os.makedirs(os.path.dirname(resaved), exist_ok=True)
        print("== %s" % v)
        for k, val in open_and_resave(band, resaved).items():
            print("  %-10s %s" % (k, val))
        if os.path.isdir(resaved):
            for k, val in read_back(resaved).items():
                print("  %-10s %s" % (k, val))
            folder = os.path.join(resaved, "Media", "Audio Files")
            for n in sorted(os.listdir(folder)) if os.path.isdir(folder) else []:
                label = known.get(wav_data(os.path.join(folder, n)))
                if label:
                    print("  %-10s %s holds %s" % ("samples", n, label))


def main() -> None:
    args = sys.argv[1:]
    if args[:1] == ["--open"] and len(args) >= 2:
        check(args[1], args[2:] or [v for v in VARIANTS if not v.startswith("bad-")])
    elif len(args) >= 2 and not args[0].startswith("-"):
        which = args[2:] or VARIANTS
        unknown = set(which) - set(VARIANTS)
        if unknown:
            raise SystemExit("unknown variants: %s" % ", ".join(sorted(unknown)))
        build(args[0], args[1], which)
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
