#!/usr/bin/env python3
"""Build a .band of a new shape from one GarageBand saved (§13).

    python3 -I build_band.py BASE.band OUT_DIR [VARIANT ...]
    python3 -I build_band.py --open OUT_DIR [VARIANT ...]

The first form writes OUT_DIR/<variant>.band for each variant. BASE must be a
project GarageBand 10.4.14 saved with at least one MIDI track holding a region
and one audio track holding a region, such as §10.1's `c3-copy`. Its own
tracks are the exemplars: every record added is a copy of one of BASE's,
changed where §11 says the links and counters are.

    copy     BASE's records split and joined again, nothing changed (a control)
    regions  no new track: a second MIDI region at bar 5 on the MIDI track, two
             new audio files on the audio track at bars 7 and 9, and a second
             region of the first new file at bar 11
    midi1    one MIDI track added, with one region of four notes at bar 1
    audio1   one audio track added, with one new audio file at bar 1
    big      8 MIDI tracks and 4 audio tracks added, 14 tracks in all: each
             MIDI track has regions at bars 1 and 5, each audio track a new
             file at bar 1 and a second region of it at bar 5, and the last
             audio track a second file at bar 9

What a track brings, as GarageBand adds one (§10.4, §11, §13.2):

    Envi        a copy of the exemplar track's, at the next free slot, named
                after the track; its strip number and strip id set
    AuCO, AuCU  a copy of the exemplar's strip and plug-ins, put after the last
                strip of its kind in use; every later strip renumbered, and the
                Envis that name them; AuCn 9's counts and its list grown
    Trak        an arrange row before the master, and an automation row after
                the exemplar's, with fresh ids
    sequences   the track's *Automation sequence, and one sequence per MIDI
                region, at new class-23 indexes with new, distinct counters
    events      an 80-byte placement per region in the arrange, one per
                automation lane in the automation folder
    AuFl, AuRg  per audio file and region, copies of the exemplar's, with the
                file's name, size, frames and format, and a GenM per region
    Song        a row in both registry tables (§13.2) for every new Envi,
                sequence, audio file and GenM
    MetaData    NumberOfTracks and AudioFiles

The ids GarageBand reads as version-1 UUIDs are made fresh in every copied
record, the same new id for the same old one within one track, so a strip's
id in its Envi and in its AuCO stay equal. Names are padded so the fields
after them start at an even offset (§11.4). The rest of BASE's package is
copied unchanged; the WAVs are made here, tones of a pitch per file.

The second form is wrap_projectdata.py's loop (§12.1): open each project in
GarageBand, save it to OUT_DIR/resaved/, close it without saving, and read
back what GarageBand kept, with inspect_band.py's link checks.

Standard library only, and MIT-clean: no code of LogicProFormatWriter or of
any GPL project is in it. Where it follows LogicProFormatWriter's findings
(a MIDI region's start, the placement's bar field), §13 says so.
"""

from __future__ import annotations

import math
import os
import plistlib
import random
import re
import shutil
import struct
import sys
import uuid
import wave

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # -I leaves it out
import inspect_band as ib  # noqa: E402
import patch_band_template as pt  # noqa: E402

BAR = 3840  # a 4/4 bar in ticks of 960 a quarter
STRIPS = 0x240000  # the index of AuCn 9, which holds the channel strips (§11.1)
REG_END = b"\x23\0\0\0\xff\xff\xff\xff"  # the row that ends each registry table
ENVI_NAME, MSEQ_NAME, AURG_NAME = 0xC2, 0x34, 0x24 + pt.AURG_NAME
AUCO_NAME = 0x61  # a strip's own name, 16 bytes NUL-padded: "Inst 1", "Audio 2"
AUCN_TOTAL, AUCN_KIND = 0x24 + 0x1A, {"Audio": 0x24 + 0x1C, "Inst": 0x24 + 0x22}
ROW_ENVI, ROW_ID, ROW_STATE = 0x24 + 0x06, 0x24 + 0x18, 0x24 + 0x28  # a Trak row's fields
SELECTED = 0x20


def u16(b, o): return struct.unpack_from("<H", b, o)[0]
def u32(b, o): return struct.unpack_from("<I", b, o)[0]
def tag(r): return bytes(r[:4][::-1]).decode("latin-1")
def cls(r): return u16(r, 6)
def index(r): return u32(r, 8)


def name(r: bytearray, at: int) -> str:
    return bytes(r[at + 2:at + 2 + u16(r, at)]).decode("utf-8", "replace")


def set_name(r: bytearray, at: int, new: str) -> None:
    """Replace a u16-length name, keeping the fields after it at an even offset (§11.4)."""
    old_end = ib.after_name(bytes(r), at)
    raw = new.encode("utf-8")
    pad = b"\0" if (at + 2 + len(raw)) & 1 else b""
    r[at:old_end] = struct.pack("<H", len(raw)) + raw + pad


def new_uuid() -> bytes:
    """A version-1 UUID as GarageBand writes them (§10.3), with a random node."""
    return uuid.uuid1(node=random.getrandbits(48) | 1 << 40).bytes


class Project:
    def __init__(self, band: str):
        self.band = band
        data = ib.project_data_of(band)
        chunks, end = ib.walk(data)
        if end != len(data):
            raise SystemExit("%s: ProjectData does not walk to its end" % band)
        self.head = bytearray(data[:ib.FILE_HEADER])
        self.recs = [bytearray(data[c.offset:c.offset + ib.CHUNK_HEADER + len(c.body)]) for c in chunks]
        self.meta = ib.plist(os.path.join(ib.alternatives(band)[0], "MetaData.plist"))
        self.wavs: dict[str, str] = {}  # Media/Audio Files/<name> -> a WAV made here
        song = self.recs[0]
        self.markers = {bytes(song[k + 12:k + 16]) for k in range(0, len(song) - 24, 4)
                        if u32(song, k) in (20, 23) and song[k + 14:k + 16] == b"\x11\xf1"}
        self.counter = max(u32(r, 0x2C) for r in self.recs if tag(r) == "MSeq")
        self.added: dict[int, list[int]] = {}  # the slots of the tracks added, by kind
        self.frames: dict[int, int] = {}  # the frames of each audio file added, by index
        self.log: list[str] = []

    # -- finding things ----------------------------------------------------------

    def find(self, t: str, c: int | None = None, i: int | None = None) -> list[int]:
        return [k for k, r in enumerate(self.recs) if tag(r) == t and (c is None or cls(r) == c)
                and (i is None or index(r) == i)]

    def sequence(self, c: int, i: int) -> tuple[int, int]:
        """(first, last) record positions of the sequence MSeq, rows, end Trak, EvSq."""
        k = self.find("MSeq", c, i)[0]
        j = k + 1
        while tag(self.recs[j]) == "Trak":
            j += 1
        return k, j

    def rows(self, i: int) -> list[int]:
        k, j = self.sequence(23, i << 18)
        return [n for n in range(k + 1, j) if self.recs[n][0x24:]]

    def placements(self, i: int) -> list[bytearray]:
        evsq = self.recs[self.sequence(23, i << 18)[1]]
        return [bytearray(e) for e in ib.placements(bytes(evsq[0x24:]))]

    def envi(self, slot: int) -> bytearray:
        return self.recs[self.find("Envi", 20, slot << 16)[0]]

    def strip_of(self, e: bytearray) -> int:
        return u16(e, ib.after_name(bytes(e), ENVI_NAME)) - 1

    def strips(self) -> list[tuple[int, int]]:
        """(first, end) record positions of each strip in AuCn 9: its AuCO and its AuCUs."""
        out = []
        for k, r in enumerate(self.recs):
            if tag(r) == "AuCO" and index(r) == STRIPS:
                j = k + 1
                while tag(self.recs[j]) == "AuCU":
                    j += 1
                out.append((k, j))
        return out

    # -- ids, counters, the registry --------------------------------------------

    def fresh_ids(self, recs: list[bytearray], ids: dict[bytes, bytes]) -> None:
        for r in recs:
            for m in self.markers:
                for hit in re.finditer(re.escape(m), bytes(r)):
                    k = hit.start() - 4
                    old = bytes(r[k:k + 16])
                    if k >= 0 and len(old) == 16 and old[6] >> 4 == 1 and old[8] & 0xC0 == 0x80:
                        r[k:k + 16] = ids.setdefault(old, new_uuid())

    def next_counter(self) -> int:
        self.counter += 1
        return self.counter

    def register(self, c: int, slot: int) -> None:
        """Add (class, slot) to both Song tables: its id, then its time stamp (§13.2)."""
        song = self.recs[0]
        u = uuid.UUID(bytes=new_uuid())
        for stride, row in ((24, struct.pack("<II", c, slot) + u.bytes),
                            (16, struct.pack("<IIQ", c, slot, u.time))):
            ends = [k for k in range(0, len(song) - 8, 4) if song[k:k + 8] == REG_END]
            at = ends[0 if stride == 24 else 1]
            while (u32(song, at - stride), u32(song, at - stride + 4)) > (c, slot):
                at -= stride
            song[at:at] = row

    # -- strips -------------------------------------------------------------------

    def add_strip(self, exemplar: int, ids: dict) -> int:
        """Copy strip `exemplar`, put it after the last strip of its kind in use, renumber."""
        strips = self.strips()
        kind = bytes(self.recs[strips[exemplar][0]][AUCO_NAME:AUCO_NAME + 16]).split(b"\0")[0]
        kind = kind.decode().rsplit(" ", 1)[0]
        # In use: a track's strip. Preview and the like hold spares, and GarageBand
        # puts a new strip before those (c0-base -> c1-audio, two -> three).
        used = {self.strip_of(self.envi(u32(self.recs[k], ROW_ENVI) >> 16)) for k in self.rows(1)}
        same = [n for n, (k, _) in enumerate(strips)
                if bytes(self.recs[k][AUCO_NAME:]).split(b"\0")[0].decode().rsplit(" ", 1)[0] == kind]
        last_used = max(n for n in same if n in used)
        k, j = strips[exemplar]
        block = [bytearray(r) for r in self.recs[k:j]]
        self.fresh_ids(block, ids)
        at = strips[last_used][1]
        self.recs[at:at] = block
        new = last_used + 1
        # Every strip from `new` on moves up one; the Envis that name them follow.
        for e in (self.recs[n] for n in self.find("Envi", 20)):
            a = ib.after_name(bytes(e), ENVI_NAME)
            if u16(e, a) - 1 >= new:
                struct.pack_into("<H", e, a, u16(e, a) + 1)
        for n, (k, j) in enumerate(self.strips()):
            for r in self.recs[k:j]:
                struct.pack_into("<H", r, 0x0E, n)
        ordinal = 0
        for k, _ in self.strips():
            r = self.recs[k]
            if bytes(r[AUCO_NAME:]).split(b"\0")[0].decode().rsplit(" ", 1)[0] == kind:
                struct.pack_into("<H", r, 0x24 + 0x06, ordinal)
                r[AUCO_NAME:AUCO_NAME + 16] = ("%s %d" % (kind, ordinal + 1)).encode().ljust(16, b"\0")
                ordinal += 1
        aucn = self.recs[self.find("AuCn", 14, STRIPS)[0]]
        for off in (AUCN_TOTAL, AUCN_KIND[kind]):
            struct.pack_into("<H", aucn, off, u16(aucn, off) + 1)
        aucn += struct.pack("<I", 1)
        return new

    # -- rows, sequences, events ------------------------------------------------

    def insert_row(self, folder: int, place: int, row: bytearray) -> None:
        """Put `row` at `place` among a folder's rows; later rows and their events move down."""
        rows = self.rows(folder)
        at = rows[place] if place < len(rows) else self.sequence(23, folder << 18)[1] - 1
        self.recs[at:at] = [row]
        for n, k in enumerate(self.rows(folder)):
            struct.pack_into("<I", self.recs[k], 0x12, n)
        self.edit_events(folder, lambda e: struct.pack_into("<H", e, 0x14, u16(e, 0x14) + 1)
                         if u16(e, 0x14) > place else None)
        if folder == 1:  # the arrange's height is 0x3C a row (§11.3)
            m = self.recs[self.sequence(23, 1 << 18)[0]]
            struct.pack_into("<H", m, ib.after_name(bytes(m), MSEQ_NAME) + 0x0A, ib.ROW_HEIGHT * len(self.rows(1)))

    def edit_events(self, folder: int, fn, add: list[bytes] = ()) -> None:
        """Apply `fn` to each 80-byte placement of a folder's EvSq, add `add`, sort by time."""
        k = self.sequence(23, folder << 18)[1]
        r = self.recs[k]
        body = bytes(r[0x24:])
        events, i = [], 0
        while i < len(body):
            n = 1
            while i + (n + 1) * ib.EVENT <= len(body) and body[i + n * ib.EVENT + 7] & 0x80:
                n += 1
            events.append(bytearray(body[i:i + n * ib.EVENT]))
            i += n * ib.EVENT
        tail = events.pop()  # the 0xf1 row at 0x3fffffff
        for e in events:
            if e[0] in (0x20, 0x24) and len(e) == ib.PLACEMENT:
                fn(e)
        events += [bytearray(e) for e in add]
        events.sort(key=lambda e: (u32(e, 4), u16(e, 0x14) if len(e) == ib.PLACEMENT else 0))
        r[0x24:] = b"".join(events) + tail

    def add_sequence(self, c: int, src: int, ids: dict, name_: str | None = None) -> tuple[int, list[bytearray]]:
        """Copy class-23 sequence `src` to the next free index, after the last one, with a new counter."""
        k, j = self.sequence(c, src)
        block = [bytearray(r) for r in self.recs[k:j + 1]]
        self.fresh_ids(block, ids)
        new = max(index(self.recs[n]) for n in self.find("MSeq", 23)) + (1 << 18)
        counter = self.next_counter()
        for r in block:
            struct.pack_into("<I", r, 8, new)
        struct.pack_into("<I", block[0], 0x2C, counter)
        struct.pack_into("<I", block[-1], 0x0E, counter)
        if name_ is not None:
            set_name(block[0], MSEQ_NAME, name_)
        last = max(self.find("EvSq", 23))
        self.recs[last + 1:last + 1] = block
        self.register(23, new >> 16)
        return new, block

    # -- tracks ---------------------------------------------------------------------

    def exemplar(self, kind: int) -> tuple[int, int, bytearray]:
        """(arrange row number, Envi slot, first placement) of BASE's first track with a `kind` event."""
        for e in self.placements(1):
            if e[0] == kind:
                row = u16(e, 0x14) - 1
                return row, u32(self.recs[self.rows(1)[row]], ROW_ENVI) >> 16, e
        raise SystemExit("%s has no track with a 0x%02x region" % (self.band, kind))

    def add_track(self, kind: int, title: str) -> tuple[int, int, int, bytearray]:
        """A track like BASE's first MIDI (0x20) or audio (0x24) track: (slot, row, Envi index, its placement)."""
        ex_row, ex_slot, ex_event = self.exemplar(kind)
        ids: dict[bytes, bytes] = {}
        ex_envi = self.envi(ex_slot)
        tracks = [u32(self.recs[k], ROW_ENVI) >> 16 for k in self.rows(1) if u16(self.recs[k], 0x24) == 1]
        slot = max(index(self.recs[k]) >> 16 for k in self.find("Envi", 20)) + 4
        strip = self.add_strip(self.strip_of(ex_envi), ids)
        e = bytearray(ex_envi)
        self.fresh_ids([e], ids)
        struct.pack_into("<I", e, 8, slot << 16)
        struct.pack_into("<H", e, 0x34, slot)
        set_name(e, ENVI_NAME, title)
        struct.pack_into("<H", e, ib.after_name(bytes(e), ENVI_NAME), strip + 1)
        xs = [u16(self.envi(s), 0x76) for s in tracks if self.find("Envi", 20, s << 16)]
        struct.pack_into("<H", e, 0x76, max(xs) + 0x42)
        for s in tracks:  # the newest channel carries 1 at rec +0x51, the others 3 (§13.2)
            other = self.envi(s) if self.find("Envi", 20, s << 16) else None
            if other is not None and other[0x51] == 1:
                other[0x51] = 3
        e[0x51] = 1
        self.recs.insert(max(self.find("Envi", 20)) + 1, e)
        self.register(20, slot)
        # The arrange row, before the master.
        row = bytearray(self.recs[self.rows(1)[ex_row]])
        self.fresh_ids([row], ids)
        struct.pack_into("<I", row, ROW_ENVI, slot << 16)
        row[ROW_STATE] &= ~SELECTED & 0xFF
        place = len(self.rows(1)) - 1
        self.insert_row(1, place, row)
        # The automation lane: a row after the exemplar's, its *Automation sequence, its event.
        auto = [(n, k) for n, k in enumerate(self.rows(2)) if u32(self.recs[k], ROW_ENVI) >> 16 == ex_slot]
        lane = [ev for ev in self.placements(2) if auto and u16(ev, 0x14) == auto[0][0] + 1]
        if auto and lane:
            n, k = auto[0]
            arow = bytearray(self.recs[k])
            seq, block = self.add_sequence(23, u32(lane[0], 0x20) << 16, ids)
            m = block[0]  # an *Automation sequence names its channel as a region does (§11.2)
            struct.pack_into("<I", m, ib.after_name(bytes(m), MSEQ_NAME) + 0xCA, slot << 16)
            self.fresh_ids([arow], ids)
            struct.pack_into("<I", arow, ROW_ENVI, slot << 16)
            n_new = n + 1 + len(self.added.get(kind, ()))  # after the exemplar's and earlier copies
            self.insert_row(2, n_new, arow)
            ev = bytearray(lane[0])
            struct.pack_into("<I", ev, 0x10, slot)
            struct.pack_into("<H", ev, 0x14, n_new + 1)
            struct.pack_into("<I", ev, 0x20, seq >> 16)
            self.edit_events(2, lambda e_: None, [bytes(ev)])
            m = self.recs[self.sequence(23, 2 << 18)[0]]
            a = ib.after_name(bytes(m), MSEQ_NAME) + 0x0A
            struct.pack_into("<H", m, a, u16(m, a) + ib.ROW_HEIGHT)
        self.added.setdefault(kind, []).append(slot)
        self.log.append("%s track %r: Envi 0x%x, strip %d, row %d" % (
            "MIDI" if kind == 0x20 else "audio", title, slot, strip, place + 1))
        return slot, place + 1, slot << 16, bytearray(ex_event)

    def add_midi_region(self, slot: int, row: int, bar: int, title: str, notes: list[tuple]) -> None:
        """A region at `bar` (from 1) on a MIDI track: a copy of the exemplar's sequence, notes rebuilt."""
        _, ex_slot, ex_event = self.exemplar(0x20)
        src = u32(ex_event, 0x20) << 16
        ids: dict[bytes, bytes] = {}
        new, block = self.add_sequence(23, src, ids, title)
        m, evsq = block[0], block[-1]
        a = ib.after_name(bytes(m), MSEQ_NAME)
        start = (bar - 1) * BAR
        length = max(BAR, -(-max(t + n for t, _, _, n in notes) // BAR) * BAR)
        struct.pack_into("<I", m, a + 0x3C, length)
        struct.pack_into("<I", m, a + 0xCA, slot << 16)
        struct.pack_into("<I", m, a + 0xE0, start)  # LPFW's start, zero-based (§9.6.2)
        rows = [bytes(evsq[0x24 + i:0x24 + i + 16]) for i in range(0, len(evsq) - 0x24, 16)]
        program = [r for r in rows if r[0] & 0xF0 == 0xC0][:1]
        head = next(r for r in rows if r[0] & 0xF0 == 0x90)
        tail2 = rows[rows.index(head) + 1]
        body = b"".join(program)
        for t, pitch, vel, n in notes:  # region-relative, from 38400 (§2.6, §9.6.3)
            h, h2 = bytearray(head), bytearray(tail2)
            struct.pack_into("<I", h, 4, ib.BAR_ONE + t)
            h[11], h[12] = vel, pitch
            struct.pack_into("<I", h2, 12, n)
            body += bytes(h) + bytes(h2)
        evsq[0x24:] = body + rows[-1]
        ev = bytearray(ex_event)
        struct.pack_into("<I", ev, 4, ib.REGION_BAR_ONE + start)
        struct.pack_into("<I", ev, 0x0C, u32(ex_event, 0x0C) + (bar - 1) * 0x100)  # LPFW's F4->F5
        struct.pack_into("<I", ev, 0x10, slot)
        struct.pack_into("<H", ev, 0x14, row)
        struct.pack_into("<I", ev, 0x20, new >> 16)
        self.edit_events(1, lambda e: None, [bytes(ev)])
        self.log.append("  MIDI region %r at bar %d: sequence %d, %d notes" % (title, bar, new >> 18, len(notes)))

    def add_audio_file(self, wav: str, filename: str) -> int:
        """An AuFl for `wav`, as Media/Audio Files/<filename>; returns its index."""
        ex = self.find("AuFl", 11)[0]
        f = bytearray(self.recs[ex])
        new = max(index(self.recs[k]) for k in self.find("AuFl", 11)) + (1 << 18)
        struct.pack_into("<I", f, 8, new)
        b = f[0x24:]
        old_end = pt.AUFL_NAME + 2 + 2 * u16(b, pt.AUFL_NAME)
        b[pt.AUFL_NAME:old_end] = struct.pack("<H", len(filename)) + filename.encode("utf-16-le")
        end = pt.AUFL_NAME + 2 + 2 * len(filename)
        with wave.open(wav, "rb") as w:
            frames, rate, ch, bits = w.getnframes(), w.getframerate(), w.getnchannels(), 8 * w.getsampwidth()
        struct.pack_into("<I", b, end + pt.AUFL_SIZE, os.path.getsize(wav))
        struct.pack_into("<III", b, end + pt.AUFL_WAVE + 8, 44, frames, 0)
        struct.pack_into("<IHH", b, end + pt.AUFL_WAVE + 0x14, rate, ch, bits)
        struct.pack_into("<I", b, end + pt.AUFL_LGWV, 0)
        b[end + pt.AUFL_PATH:end + pt.AUFL_PATH + pt.AUFL_PATH_LEN] = b"Audio Files".ljust(pt.AUFL_PATH_LEN, b"\0")
        f[0x24:] = b
        last = max(self.find("AuFl", 11) + self.find("AuRg", 11))
        self.recs.insert(last + 1, f)
        self.register(11, new >> 16)
        self.wavs[filename] = wav
        self.meta["AudioFiles"] = list(self.meta.get("AudioFiles", [])) + ["Audio Files/" + filename]
        self.frames[new] = frames
        return new

    def add_audio_region(self, slot: int, row: int, f: int, bar: int, title: str) -> None:
        """A region of the whole of audio file `f` at `bar` on an audio track, with its GenM."""
        _, _, ex_event = self.exemplar(0x24)
        ex = self.find("AuRg", 11, u32(ex_event, 0x2C) << 16)[0]
        r = bytearray(self.recs[ex])
        n = len(self.find("AuRg", 11, f))
        struct.pack_into("<I", r, 8, f)
        struct.pack_into("<I", r, 0x0E, n)
        self.fresh_ids([r], {})
        struct.pack_into("<I", r, 0x24 + pt.AURG_FRAMES, self.frames[f])
        set_name(r, AURG_NAME, title)
        last = max(k for k in self.find("AuFl", 11) + self.find("AuRg", 11) if index(self.recs[k]) == f)
        self.recs.insert(last + 1, r)
        g = bytearray(self.recs[self.find("GenM", 30)[-1]])  # loop metadata, one per region (§10.4)
        gi = max(index(self.recs[k]) for k in self.find("GenM", 30)) + (1 << 18)
        struct.pack_into("<I", g, 8, gi)
        struct.pack_into("<I", g, 0x24 + 0x0C, f >> 16)
        struct.pack_into("<I", g, 0x24 + 0x10, n)
        self.recs.insert(max(self.find("GenM", 30)) + 1, g)
        self.register(30, gi >> 16)
        ev = bytearray(ex_event)
        struct.pack_into("<I", ev, 4, ib.REGION_BAR_ONE + (bar - 1) * BAR)
        struct.pack_into("<I", ev, 0x10, slot)
        struct.pack_into("<H", ev, 0x14, row)
        struct.pack_into("<I", ev, 0x28, n)
        struct.pack_into("<I", ev, 0x2C, f >> 16)
        self.edit_events(1, lambda e: None, [bytes(ev)])
        self.log.append("  audio region %r at bar %d: file %d, region %d" % (title, bar, f >> 18, n))

    # -- writing ----------------------------------------------------------------------

    def write(self, dest: str) -> None:
        if os.path.exists(dest):
            shutil.rmtree(dest)
        shutil.copytree(self.band, dest, symlinks=True,
                        ignore=shutil.ignore_patterns("Undo Data.nosync", "WindowImage.jpg"))
        body = bytearray()
        for r in self.recs:
            struct.pack_into("<Q", r, 0x1C, len(r) - ib.CHUNK_HEADER)
            body += r
        struct.pack_into("<Q", self.head, 16, len(body))
        alt = os.path.join(dest, os.path.relpath(ib.alternatives(self.band)[0], self.band))
        with open(os.path.join(alt, "ProjectData"), "wb") as f:
            f.write(self.head + body)
        self.meta["NumberOfTracks"] = len(self.rows(1)) - 1
        with open(os.path.join(alt, "MetaData.plist"), "wb") as f:
            plistlib.dump(self.meta, f, fmt=plistlib.FMT_XML)
        folder = os.path.join(dest, "Media", "Audio Files")
        os.makedirs(folder, exist_ok=True)
        for n, src in self.wavs.items():
            shutil.copyfile(src, os.path.join(folder, n))


def tone(path: str, seconds: float, freq: float, rate: int = 44100) -> str:
    """A WAV of beeps at `freq`, two a second, so each file sounds and looks its own."""
    out = bytearray()
    for i in range(round(seconds * rate)):
        t = i / rate
        x = t % 0.5
        v = 0.3 * math.sin(2 * math.pi * freq * t) * math.exp(-6 * x)
        s = int(v * 32767)
        out += struct.pack("<hh", s, s)
    with wave.open(path, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(bytes(out))
    return path


SCALE = [60, 62, 64, 65, 67, 69, 71, 72]


def phrase(k: int) -> list[tuple]:
    """Four quarter notes, a different start per track: (tick, pitch, velocity, length)."""
    return [(q * 960, SCALE[(k + q) % 8] + 12 * (k % 2), 96, 960) for q in range(4)]


VARIANTS = ["copy", "regions", "midi1", "audio1", "big"]


def build(base: str, out: str, which: list[str]) -> None:
    os.makedirs(os.path.join(out, "media"), exist_ok=True)
    wav = {}

    def file(n: int, seconds: float) -> str:
        if n not in wav:
            wav[n] = tone(os.path.join(out, "media", "tone%02d.wav" % n), seconds, 220 * 2 ** (n / 6))
        return wav[n]

    for v in which:
        p = Project(base)
        if v == "regions":
            row, slot, _ = p.exemplar(0x20)
            p.add_midi_region(slot, row + 1, 5, "One again", phrase(3))
            row, slot, _ = p.exemplar(0x24)
            f1 = p.add_audio_file(file(1, 2.0), "Tone 1.wav")
            f2 = p.add_audio_file(file(2, 3.0), "Tone 2.wav")
            p.add_audio_region(slot, row + 1, f1, 7, "Tone 1")
            p.add_audio_region(slot, row + 1, f2, 9, "Tone 2")
            p.add_audio_region(slot, row + 1, f1, 11, "Tone 1.1")
        if v == "midi1":
            slot, row, _, _ = p.add_track(0x20, "Part 2")
            p.add_midi_region(slot, row, 1, "Part 2", phrase(1))
        if v == "audio1":
            slot, row, _, _ = p.add_track(0x24, "Stem 2")
            p.add_audio_region(slot, row, p.add_audio_file(file(1, 2.0), "Stem 2.wav"), 1, "Stem 2")
        if v == "big":
            for k in range(8):
                title = "Part %d" % (k + 2)
                slot, row, _, _ = p.add_track(0x20, title)
                p.add_midi_region(slot, row, 1, title, phrase(k))
                p.add_midi_region(slot, row, 5, title + " B", phrase(k + 4))
            for k in range(4):
                title = "Stem %d" % (k + 2)
                slot, row, _, _ = p.add_track(0x24, title)
                f = p.add_audio_file(file(k + 1, 2.0 + k % 2), title + ".wav")
                p.add_audio_region(slot, row, f, 1, title)
                p.add_audio_region(slot, row, f, 5, title + ".1")
            f = p.add_audio_file(file(9, 2.0), "Stem 5 take 2.wav")
            p.add_audio_region(slot, row, f, 9, "Stem 5 take 2")
        dest = os.path.join(out, v + ".band")
        p.write(dest)
        lines, fails = ib.links(ib.project_data_of(dest), ib.plist(os.path.join(ib.alternatives(dest)[0], "MetaData.plist")))
        print("%-8s %d tracks, %d records; links: %s" % (v, len(p.rows(1)) - 1, len(p.recs),
                                                         "every check held" if not fails else "; ".join(fails[:5])))
        for line in p.log:
            print("    " + line)


def main() -> int:
    args = sys.argv[1:]
    if args[:1] == ["--open"] and len(args) >= 2:
        import wrap_projectdata as wp
        return wp.try_open(args[1], args[2:])
    if len(args) >= 2 and not args[0].startswith("-"):
        which = args[2:] or VARIANTS
        unknown = set(which) - set(VARIANTS)
        if unknown:
            raise SystemExit("unknown variants: %s" % ", ".join(sorted(unknown)))
        build(args[0], args[1], which)
        return 0
    print(__doc__.split("\n\n")[1], file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
