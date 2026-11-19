#!/usr/bin/env python3
"""Take apart a Guitar Pro file (docs/research/guitar-pro/README.md).

    python3 -I inspect_gp.py FILE.gp3|.gp4|.gp5 [--beats] [--encoding cp1251]   # §2, walked to the end
    python3 -I inspect_gp.py FILE.gpx [--xml OUT.gpif]        # §3: BCFZ, BCFS, score.gpif

Written for this repository from the README's description and the samples'
bytes (MIT, as the studio). Standard library only. It walks every section of
a GP3/4/5 file and fails loudly on the first byte it cannot place, so a walk
that ends exactly at the file's end checks the layout of §2. For a .gpx it
unpacks the container and prints the GPIF's counts and the tracks' tunings.
"""

from __future__ import annotations

import struct
import sys
import xml.etree.ElementTree as ET


class Reader:
    def __init__(self, data: bytes, encoding: str = "cp1252"):
        self.d = data
        self.p = 0
        self.encoding = encoding

    def take(self, n: int) -> bytes:
        if self.p + n > len(self.d):
            raise EOFError(f"read past the end at {self.p:#x} (+{n})")
        b = self.d[self.p : self.p + n]
        self.p += n
        return b

    def u8(self) -> int:
        return self.take(1)[0]

    def i8(self) -> int:
        return struct.unpack("<b", self.take(1))[0]

    def i16(self) -> int:
        return struct.unpack("<h", self.take(2))[0]

    def i32(self) -> int:
        return struct.unpack("<i", self.take(4))[0]

    def f64(self) -> float:
        return struct.unpack("<d", self.take(8))[0]

    def boolean(self) -> bool:
        return self.u8() != 0

    def text(self, raw: bytes) -> str:
        return raw.decode(self.encoding, errors="replace")

    def byte_string(self, size: int) -> str:
        """A length byte, then a field of `size` bytes."""
        n = self.u8()
        raw = self.take(size)
        return self.text(raw[: min(n, size)])

    def int_string(self) -> str:
        """An int32 length, then the characters."""
        return self.text(self.take(self.i32()))

    def int_byte_string(self) -> str:
        """An int32 (the length + 1), a length byte, then the characters."""
        total = self.i32()
        n = self.u8()
        raw = self.take(total - 1)
        return self.text(raw[:n])


class GP345:
    def __init__(self, data: bytes, beats: bool, encoding: str = "cp1252"):
        self.r = Reader(data, encoding)
        self.show_beats = beats
        self.stats: dict[str, int] = {}

    def count(self, what: str) -> None:
        self.stats[what] = self.stats.get(what, 0) + 1

    def walk(self) -> None:
        r = self.r
        self.version_text = r.byte_string(30)
        major = int(self.version_text.split("v")[1][0])
        minor = int(self.version_text.split(".")[-1])
        self.v = major
        self.v510 = major == 5 and minor >= 10
        print(f"version   {self.version_text!r}")
        names = ["title", "subtitle", "artist", "album", "words"]
        names += ["music"] if self.v == 5 else []
        names += ["copyright", "tab", "instructions"]
        for n in names:
            print(f"{n:<12}{r.int_byte_string()!r}")
        for _ in range(r.i32()):
            print(f"notice      {r.int_byte_string()!r}")
        if self.v < 5:
            print(f"triplet     {r.boolean()}")
        if self.v >= 4:
            track = r.i32()
            lines = [(r.i32(), r.int_string()) for _ in range(5)]
            used = [(bar, t[:30]) for bar, t in lines if t]
            print(f"lyrics      track {track}, {used}")
        if self.v510:
            print(f"rse master  volume {r.i32()}, {r.i32()}, eq {list(r.take(11))}")
        if self.v == 5:
            page = [r.i32() for _ in range(7)]
            flags = r.i16()
            heads = [r.int_byte_string() for _ in range(10)]
            print(f"page        {page} flags {flags:#x} {heads[:3]}…")
            print(f"tempo name  {r.int_byte_string()!r}")
        self.tempo = r.i32()
        print(f"tempo       {self.tempo}")
        if self.v510:
            print(f"hide tempo  {r.boolean()}")
        if self.v == 3:
            print(f"key         {r.i32()}")
        else:
            print(f"key         {r.i8()} octave-ish {list(r.take(4 if self.v == 5 else 4))}")
        channels = []
        for _ in range(64):
            prog = r.i32()
            vol, bal, cho, rev, pha, tre = (r.u8() for _ in range(6))
            r.take(2)
            channels.append((prog, vol, bal))
        self.channels = channels
        print(f"channels    {channels[:4]}…")
        if self.v == 5:
            print(f"directions  {[r.i16() for _ in range(19)]}")
            print(f"reverb      {r.i32()}")
        bars, tracks = r.i32(), r.i32()
        print(f"bars {bars} tracks {tracks} at {r.p:#x}")
        self.headers = [self.master_bar(i) for i in range(bars)]
        self.tracks = [self.track(i) for i in range(tracks)]
        if self.v == 5:
            gap = 1 if self.v510 else 2
            skipped = r.take(gap)
            print(f"after tracks skip {list(skipped)}")
        print(f"bars start at {r.p:#x}")
        for b in range(bars):
            for t in range(tracks):
                self.measure(b, t)
        rest = r.d[r.p :]
        whole = not rest or (self.v < 5 and rest == bytes(4))  # §2.11: GP3/GP4 end in 4 zeros
        print(f"end at {r.p:#x} of {len(r.d):#x}, {len(rest)} left" + ("  OK" if whole else "  LEFT OVER"))
        print("counts", dict(sorted(self.stats.items())))

    def master_bar(self, i: int) -> dict:
        r = self.r
        if self.v == 5 and i > 0:
            r.take(1)
        f = r.u8()
        h = {"flags": f}
        if f & 0x01:
            h["num"] = r.i8()
        if f & 0x02:
            h["den"] = r.i8()
        if f & 0x04:
            h["repeat_start"] = True
        if f & 0x08:
            h["repeat_end"] = r.i8()
        if self.v < 5:
            if f & 0x10:
                h["alternate"] = r.u8()
            if f & 0x20:
                h["marker"] = (r.int_byte_string(), list(r.take(4)))
            if f & 0x40:
                h["key"] = (r.i8(), r.i8())
        else:
            if f & 0x20:
                h["marker"] = (r.int_byte_string(), list(r.take(4)))
            if f & 0x40:
                h["key"] = (r.i8(), r.i8())
            if f & 0x10:
                h["alternate"] = r.u8()
            if f & 0x03:
                h["beams"] = list(r.take(4))
            if not f & 0x10:
                r.take(1)
            h["triplet"] = r.u8()
        if f & 0x80:
            h["double_bar"] = True
        if f & ~0x83 or i < 3:
            shown = {k: v for k, v in h.items() if k != "flags"}
            print(f"  bar {i + 1}: {shown}")
        return h

    def track(self, i: int) -> dict:
        r = self.r
        if self.v == 5 and (i == 0 or not self.v510):
            r.take(1)
        f = r.u8()
        name = r.byte_string(40)
        strings = r.i32()
        tuning = [r.i32() for _ in range(7)][:strings]
        port, channel, effects, frets, capo = (r.i32() for _ in range(5))
        color = list(r.take(4))
        program = self.channels[(port - 1) * 16 + channel - 1][0]
        t = {"name": name, "flags": f, "tuning": tuning, "program": program, "port": port, "channel": channel,
             "effects_channel": effects, "frets": frets, "capo": capo}
        if self.v == 5:
            f2 = r.i16()
            accent = r.u8()
            bank = r.u8()
            humanize = r.u8()
            r.take(12 + 12)
            inst = (r.i32(), r.i32(), r.i32())
            if self.v510:
                eff = r.i32()
                eq = list(r.take(4))
                rse = (r.int_byte_string(), r.int_byte_string())
            else:
                eff = r.i16()
                r.take(1)
                rse = None
            t.update(flags2=f2, bank=bank, rse=(inst, eff, rse))
        print(f"  track {i + 1}: {t}")
        return t

    def measure(self, b: int, t: int) -> None:
        r = self.r
        for voice in range(2 if self.v == 5 else 1):
            n = r.i32()
            if not 0 <= n < 256:
                raise ValueError(f"bar {b + 1} track {t + 1} voice {voice + 1}: {n} beats at {r.p - 4:#x}")
            for k in range(n):
                self.beat(b, t, voice, k)
        if self.v == 5 and r.p < len(r.d):
            r.u8()  # between bars; the file's last bar has none (README §2.11)

    def beat(self, b: int, t: int, voice: int, k: int) -> None:
        r = self.r
        at = r.p
        f = r.u8()
        status = r.u8() if f & 0x40 else 1
        dur = r.i8()
        if status not in (0, 1, 2) or not -2 <= dur <= 4:
            raise ValueError(f"bar {b + 1} track {t + 1}: beat {k} at {at:#x}: flags {f:#x} status {status} duration {dur}")
        tuplet = r.i32() if f & 0x20 else None
        if f & 0x02:
            self.chord()
        text = r.int_byte_string() if f & 0x04 else None
        if f & 0x08:
            self.beat_effects()
        if f & 0x10:
            self.mix_table()
        strings = r.u8()
        if status == 0:
            self.count("beat empty")
        elif status == 2:
            self.count("beat rest")
        if tuplet:
            self.count(f"tuplet {tuplet}")
        if text:
            self.count("beat text")
        notes = []
        for s in range(7):
            if strings & (1 << (6 - s)):
                notes.append((s + 1, self.note()))
        if self.v == 5:
            f2 = r.i16()
            if f2 & 0x0800:
                r.u8()
        if self.show_beats and t == 0 and b < 4:
            print(f"    b{b + 1} t{t + 1} v{voice + 1} #{k} @{at:#x} f{f:#04x} dur {dur} dot {bool(f & 1)} {notes}")

    def chord(self) -> None:
        r = self.r
        self.count("chord")
        if self.v == 3:
            new = r.u8()
            if not new:
                r.int_byte_string()
                first = r.i32()
                if first:
                    r.take(6 * 4)
                return
            # Not met in the samples (README §2.16): taken from descriptions.
            r.take(1 + 3 + 4 * 5 + 1)
            r.byte_string(20)
            r.take(2 + 4 * 3 + 4 + 6 * 4 + 4 + 4 * 6 + 7 + 1)
            return
        new = r.u8()
        if not new:
            r.int_byte_string()
            first = r.i32()
            if first:
                r.take(6 * 4)
            return
        r.take(16)  # sharp, 3 blank, root, type, extension, bass (int), tonality (int), add
        r.byte_string(21)
        r.take(4)  # fifth, ninth, eleventh, blank
        r.i32()  # first fret
        r.take(7 * 4)  # frets, -1 for a string not played
        r.take(32)  # barres (count, 5 frets, 5 starts, 5 ends), 7 omissions, blank, 7 fingers, show

    def beat_effects(self) -> None:
        r = self.r
        f1 = r.u8()
        f2 = r.u8() if self.v >= 4 else 0
        if f1 & 0x01:
            self.count("vibrato")
        if self.v == 3:
            if f1 & 0x20:
                kind = r.u8()
                r.i32()
                self.count(f"slap/tremolo {kind}")
        else:
            if f1 & 0x20:
                self.count(f"slap {r.u8()}")
            if f2 & 0x04:
                self.count("tremolo bar")
                self.bend()
        if f1 & 0x40:
            self.count("stroke")
            r.take(2)
        if self.v >= 4 and f2 & 0x02:
            self.count("pickstroke")
            r.u8()

    def bend(self) -> None:
        r = self.r
        r.u8()
        r.i32()
        for _ in range(r.i32()):
            r.i32()
            r.i32()
            r.u8()

    def mix_table(self) -> None:
        r = self.r
        self.count("mix table")
        inst = r.i8()
        if self.v == 5:
            r.take(16)
        vals = [r.i8() for _ in range(6)]  # volume balance chorus reverb phaser tremolo
        if self.v == 5:
            r.int_byte_string()
        tempo = r.i32()
        if tempo >= 0:
            self.count("tempo change")
        for v in vals:
            if v >= 0:
                r.u8()
        if tempo >= 0:
            r.u8()
            if self.v510:
                r.u8()
        if self.v >= 4:
            r.u8()
        if self.v == 5:
            r.u8()
            if self.v510:
                r.int_byte_string()
                r.int_byte_string()

    def note(self) -> tuple:
        r = self.r
        f = r.u8()
        kind = r.u8() if f & 0x20 else 1
        if f & 0x01 and self.v < 5:
            r.take(2)
        vel = r.i8() if f & 0x10 else None
        fret = r.i8() if f & 0x20 else None
        if f & 0x80:
            r.take(2)
        if self.v == 5:
            if f & 0x01:
                r.f64()
            r.u8()
        if kind == 2:
            self.count("tie")
        if kind == 3:
            self.count("dead")
        if f & 0x04:
            self.count("ghost")
        if f & 0x02:
            self.count("heavy accent" if self.v == 5 else f"flag 0x02 (GP3/4) with kind {kind}")
        if f & 0x40:
            self.count("accent")
        if f & 0x08:
            self.note_effects()
        return (fret, kind, vel)

    def note_effects(self) -> None:
        r = self.r
        f1 = r.u8()
        f2 = r.u8() if self.v >= 4 else 0
        if f1 & 0x01:
            self.count("bend")
            self.bend()
        if f1 & 0x02:
            self.count("hammer")
        if f1 & 0x08:
            self.count("let ring")
        if self.v == 3 and f1 & 0x04:
            self.count("slide")
        if f1 & 0x10:
            self.count("grace")
            r.take(5 if self.v == 5 else 4)
        if f2 & 0x01:
            self.count("staccato")
        if f2 & 0x02:
            self.count("palm mute")
        if f2 & 0x04:
            self.count("tremolo picking")
            r.u8()
        if f2 & 0x08:
            self.count(f"slide {r.u8()}")
        if f2 & 0x10:
            h = r.u8()
            self.count(f"harmonic {h}")
            if self.v == 5 and h == 2:
                r.take(3)
            if self.v == 5 and h == 3:
                r.take(1)
        if f2 & 0x20:
            self.count("trill")
            r.take(2)
        if f2 & 0x40:
            self.count("vibrato (note)")


# --- GPX -----------------------------------------------------------------


class Bits:
    def __init__(self, data: bytes, start: int):
        self.d = data
        self.pos = start * 8

    def bit(self) -> int:
        byte = self.d[self.pos >> 3]
        b = (byte >> (7 - (self.pos & 7))) & 1
        self.pos += 1
        return b

    def high_first(self, n: int) -> int:
        v = 0
        for _ in range(n):
            v = (v << 1) | self.bit()
        return v

    def low_first(self, n: int) -> int:
        v = 0
        for i in range(n):
            v |= self.bit() << i
        return v


def bcfz(data: bytes) -> bytes:
    size = struct.unpack("<i", data[4:8])[0]
    bits = Bits(data, 8)
    out = bytearray()
    while len(out) < size:
        if bits.bit():
            width = bits.high_first(4)
            offset = bits.low_first(width)
            length = bits.low_first(width)
            start = len(out) - offset
            for i in range(min(offset, length)):
                out.append(out[start + i])
        else:
            for _ in range(bits.low_first(2)):
                out.append(bits.high_first(8))
    return bytes(out[:size])


SECTOR = 0x1000


def bcfs(data: bytes) -> dict[str, bytes]:
    """The files of a BCFS image, walked from its root directory (README §3.2)."""
    assert data[:4] == b"BCFS", data[:4]
    body = data[4:]

    def i32(at: int) -> int:
        return struct.unpack("<i", body[at : at + 4])[0]

    def entry(sector: int) -> tuple[int, str, bytes]:
        at = sector * SECTOR
        kind = i32(at)
        name = body[at + 4 : at + 4 + 127].split(b"\0")[0].decode("utf-8")
        size = i32(at + 0x8C)
        blocks = []
        k = at + 0x94
        while (index := i32(k)) != 0:
            blocks.append(body[index * SECTOR : (index + 1) * SECTOR])
            k += 4
        return kind, name, b"".join(blocks)[:size]

    files = {}

    def walk(sector: int, prefix: str) -> None:
        kind, name, content = entry(sector)
        if kind == 2:
            files[prefix + name] = content
        elif kind == 1:
            for k in range(0, len(content), 4):
                child = struct.unpack("<i", content[k : k + 4])[0]
                walk(child, prefix if name == "/" else f"{prefix}{name}/")
        else:
            raise ValueError(f"sector {sector}: entry of kind {kind}")

    walk(1, "")
    return files


def gpx(path: str, data: bytes, xml_out: str | None) -> None:
    print(f"magic {data[:4]!r} size field {struct.unpack('<i', data[4:8])[0]}")
    fs = bcfz(data) if data[:4] == b"BCFZ" else data
    print(f"unpacked {len(fs)} bytes, magic {fs[:4]!r}")
    files = bcfs(fs)
    for name, body in files.items():
        print(f"  file {name!r}: {len(body)} bytes, starts {body[:24]!r}")
    gpif = files["score.gpif"]
    if xml_out:
        with open(xml_out, "wb") as f:
            f.write(gpif)
    root = ET.fromstring(gpif)
    print("root", root.tag, [c.tag for c in root])
    score = root.find("Score")
    for c in score:
        if (c.text or "").strip():
            print(f"  {c.tag:<14}{c.text.strip()[:60]!r}")
    for name in ["MasterBars", "Bars", "Voices", "Beats", "Notes", "Rhythms", "Tracks"]:
        node = root.find(name)
        print(f"{name:<11}{len(node) if node is not None else None}")
    for t in root.find("Tracks"):
        tun = t.find(".//Property[@name='Tuning']/Pitches")
        capo = t.find(".//Property[@name='CapoFret']/Fret")
        inst = t.find("Instrument")
        gm = t.find("GeneralMidi")
        print(f"  track {t.get('id')} {t.findtext('Name')!r} instrument {inst.get('ref') if inst is not None else None}"
              f" tuning {tun.text if tun is not None else None} capo {capo.text if capo is not None else None}"
              f" program {gm.findtext('Program') if gm is not None else None}")
    for a in root.findall("MasterTrack/Automations/Automation"):
        print(f"  automation {a.findtext('Type')} bar {a.findtext('Bar')} value {a.findtext('Value')}")


def main() -> None:
    args = sys.argv[1:]
    path = args[0]
    with open(path, "rb") as f:
        data = f.read()
    if data[:4] in (b"BCFZ", b"BCFS"):
        out = args[args.index("--xml") + 1] if "--xml" in args else None
        gpx(path, data, out)
    else:
        encoding = args[args.index("--encoding") + 1] if "--encoding" in args else "cp1252"
        GP345(data, "--beats" in args, encoding).walk()


if __name__ == "__main__":
    main()
