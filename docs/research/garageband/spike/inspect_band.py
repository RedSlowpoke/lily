#!/usr/bin/env python3
"""Take apart a GarageBand .band project (docs/research/garageband §2).

    python3 -I inspect_band.py PROJECT.band [PROJECT.band ...]
    python3 -I inspect_band.py --chunks PROJECT.band       # every chunk's header
    python3 -I inspect_band.py --dump MSeq PROJECT.band    # hex of one chunk type
    python3 -I inspect_band.py --diff A.band B.band        # chunks that differ, paired by tag, kind, index
    python3 -I inspect_band.py --links PROJECT.band        # tracks, strips, regions and files, and the checks

For a diff by position, byte by byte and with GarageBand's ids folded, use
diff_band.py beside it (§10.2). `--links` prints how the records point at one
another (§11) and checks every link; it exits 1 if one fails.

Reads only; standard library only. It prints the package's files, the plists
that matter, the iOS `projectData` wrapper if there is one, and a walk of the
binary `Alternatives/NNN/ProjectData`: its header, its chunks by tag, and what
§2 found readable in them (the tempo events, names, MIDI notes). What a field
means is marked by how sure §2 is of it; see the README.
"""

from __future__ import annotations

import argparse
import collections
import os
import plistlib
import re
import struct
import sys

MAGIC = b"\x23\x47\xc0\xab"  # "#G\xC0\xAB", ProjectData and every embedded song
FILE_HEADER = 24  # magic, u16 format, u16, u32, u16, u16, u64 length of the rest
CHUNK_HEADER = 36  # tag (4, reversed), 24 bytes of fields, u64 length of the body
TICKS_PER_QUARTER = 960  # Logic's internal resolution
BAR_ONE = 0x9600  # 38400: bar 1 for tempo, meter and the events in a region
REGION_BAR_ONE = 34560  # bar 1 for a region's place on a track: one 4/4 bar earlier
EVENT = 16  # an event sequence is records of 16 bytes


class Chunk:
    __slots__ = ("offset", "tag", "fields", "body")

    def __init__(self, offset: int, tag: str, fields: bytes, body: bytes):
        self.offset, self.tag, self.fields, self.body = offset, tag, fields, body

    def header(self) -> dict:
        """The 24 bytes between the tag and the length, as far as they are known."""
        f = self.fields  # chunk bytes +0x04 to +0x1C
        return {
            "version": struct.unpack_from("<H", f, 0)[0],  # per tag: Song 3, Trak 5, Envi 6, ...
            "kind": struct.unpack_from("<H", f, 2)[0],  # the spec's classId: a role for MSeq, EvSq
            "index": struct.unpack_from("<I", f, 4)[0] >> 18,  # the spec's object index
            "links": f[10:18].hex(),  # +0x0E: ffff... or a small number (EvSq, AuCO, AuCU)
            "tail": f[18:24].hex(),  # +0x16: always 020000000100 so far
        }


def read_header(data: bytes) -> dict:
    if data[:4] != MAGIC:
        raise ValueError("not a Logic/GarageBand song: magic is %s" % data[:4].hex())
    fmt, a, b, c, d, length = struct.unpack_from("<HHIHHQ", data, 4)
    return {"format": fmt, "fields": (a, b, c, d), "length": length,
            "length_ok": length == len(data) - FILE_HEADER}


def walk(data: bytes) -> tuple[list[Chunk], int]:
    """Every chunk from the end of the file header on, and where the walk stopped."""
    chunks, off = [], FILE_HEADER
    while off + CHUNK_HEADER <= len(data):
        tag = data[off:off + 4][::-1].decode("latin-1")
        (length,) = struct.unpack_from("<Q", data, off + 28)
        if not re.fullmatch(r"[A-Za-z]{4}", tag) or off + CHUNK_HEADER + length > len(data):
            break
        body = data[off + CHUNK_HEADER:off + CHUNK_HEADER + length]
        chunks.append(Chunk(off, tag, data[off + 4:off + 28], body))
        off += CHUNK_HEADER + length
    return chunks, off


def strings(body: bytes) -> list[str]:
    """Names stored as a u16 length and that many bytes of text (MSeq, Envi, Layr)."""
    out, i = [], 0
    while i + 2 < len(body):
        (n,) = struct.unpack_from("<H", body, i)
        s = body[i + 2:i + 2 + n]
        if 2 <= n <= 255 and len(s) == n and re.fullmatch(rb"[\x20-\x7e\x80-\xff]+", s):
            try:
                out.append(s.decode("utf-8"))
                i += 2 + n
                continue
            except UnicodeDecodeError:
                pass
        i += 1
    return out


MSEQ_NAME = 0x10  # body + of an MSeq's u16-length name (rec +0x34, §11.4)


def mseq_name(body: bytes) -> str | None:
    """An MSeq's name, read where it is kept, so a one-character part name
    (*V*) is found too, which `strings` skips; else the first of `strings`."""
    if len(body) >= MSEQ_NAME + 2:
        (n,) = struct.unpack_from("<H", body, MSEQ_NAME)
        s = body[MSEQ_NAME + 2:MSEQ_NAME + 2 + n]
        if 1 <= n <= 255 and len(s) == n and re.fullmatch(rb"[\x20-\x7e\x80-\xff]+", s):
            try:
                return s.decode("utf-8")
            except UnicodeDecodeError:
                pass
    return (strings(body)[:1] or [None])[0]


def position(ticks: int, origin: int = BAR_ONE) -> str:
    """A position in quarters (and ticks) from bar 1, the meter not applied."""
    q, t = divmod(ticks - origin, TICKS_PER_QUARTER)
    return "q%+d%s" % (q, "+%d" % t if t else "")


def events(body: bytes) -> list[str]:
    """The events of a sequence, one line each, as far as §2 can read them.

    An event is a head record and the records after it whose byte 7 has its
    high bit set (a head's bytes 4-7 are its position, below 2**31).
    """
    recs = [body[i:i + EVENT] for i in range(0, len(body) - EVENT + 1, EVENT)]
    groups: list[list[bytes]] = []
    for r in recs:
        if groups and r[7] & 0x80:
            groups[-1].append(r)
        else:
            groups.append([r])
    out = []
    for g in groups:
        head = g[0]
        kind = head[0]
        (pos,) = struct.unpack_from("<I", head, 4)
        if kind == 0xF1:
            out.append("end")
            break
        if kind == 0x60 and len(g) >= 2 and g[1][7] == 0x88:  # tempo: bpm x 10000 in record 2
            # (an imported tempo has 2 records, one drawn in the tempo track 4)
            (bpm,) = struct.unpack_from("<I", g[1], 0)
            out.append("tempo %s %.6g bpm" % (position(pos), bpm / 10000))
        elif kind & 0xF0 == 0x90 and len(g) >= 2:  # note: pitch, velocity, length
            (length,) = struct.unpack_from("<I", g[1], 12)
            out.append("note ch%d %s pitch %d vel %d len %d"
                       % (kind & 15, position(pos), head[12], head[11], length))
        elif kind & 0xF0 == 0xC0:  # program change
            out.append("program ch%d %s %d" % (kind & 15, position(pos), head[11]))
        elif kind == 0x24:  # an audio region's place on its track
            out.append("audio region at %s (region origin)" % position(pos, REGION_BAR_ONE))
        else:
            out.append("0x%02x@%s[%s]" % (kind, position(pos) if pos else "0",
                                          " ".join(r.hex() for r in g)))
    return out


def files(band: str) -> list[tuple[str, int]]:
    out = []
    for root, dirs, names in os.walk(band):
        dirs.sort()
        rel = os.path.relpath(root, band)
        if rel.endswith(".lproj"):
            continue  # the templates' localised names: one line for all of them
        for n in sorted(names):
            p = os.path.join(root, n)
            out.append((os.path.normpath(os.path.join(rel, n)), os.path.getsize(p)))
    lproj = [d for d in os.listdir(os.path.join(band, "Contents", "Resources"))
             if d.endswith(".lproj")] if os.path.isdir(os.path.join(band, "Contents", "Resources")) else []
    if lproj:
        out.append(("Contents/Resources/*.lproj/InfoPlist.strings (%d languages)" % len(lproj), 0))
    return out


def plist(path: str) -> dict | None:
    try:
        with open(path, "rb") as f:
            return plistlib.load(f)
    except (OSError, plistlib.InvalidFileException):
        return None


META_KEYS = ("BeatsPerMinute", "SongSignatureNumerator", "SongSignatureDenominator", "SongKey",
             "SongGenderKey", "SignatureKey", "SampleRate", "NumberOfTracks", "isTimeCodeBased",
             "AudioFiles", "UnusedAudioFiles", "PlaybackFiles", "SamplerInstrumentsFiles",
             "QuicksamplerFiles", "UltrabeatFiles", "ImpulsResponsesFiles", "Version")


def alternatives(band: str) -> list[str]:
    root = os.path.join(band, "Alternatives")
    return sorted(os.path.join(root, a) for a in os.listdir(root)) if os.path.isdir(root) else []


def ios_song(band: str) -> bytes | None:
    """The song inside the top-level `projectData` (an NSKeyedArchiver: DfLogicModel)."""
    p = plist(os.path.join(band, "projectData"))
    if not p or "$objects" not in p:
        return None
    for o in p["$objects"]:
        if isinstance(o, dict) and isinstance(o.get("NS.data"), bytes) and o["NS.data"][:4] == MAGIC:
            return o["NS.data"]
    return None


def summarize(band: str) -> None:
    print("=" * 72)
    print(band)
    print("-- files")
    for rel, size in files(band):
        print("  %9d  %s" % (size, rel))
    pkg = os.path.join(band, "Contents", "PkgInfo")
    if os.path.exists(pkg):
        print("-- PkgInfo:", open(pkg, "rb").read().decode("latin-1"))
    info = plist(os.path.join(band, "Resources", "ProjectInformation.plist"))
    if info:
        print("-- Resources/ProjectInformation.plist:", info)
    song = ios_song(band)
    if song:
        hdr = read_header(song)
        chunks, end = walk(song)
        print("-- projectData (iOS DfLogicModel wrapper): song of %d bytes, format 0x%04x, %d chunks, walk %s"
              % (len(song), hdr["format"], len(chunks), "complete" if end == len(song) else "stopped at %d" % end))
    for alt in alternatives(band):
        print("-- %s" % os.path.relpath(alt, band))
        meta = plist(os.path.join(alt, "MetaData.plist"))
        if meta:
            for k in META_KEYS:
                if k in meta:
                    print("  MetaData.%s = %r" % (k, meta[k]))
            other = sorted(set(meta) - set(META_KEYS))
            if other:
                print("  MetaData, other keys:", ", ".join(other))
        pd = os.path.join(alt, "ProjectData")
        if os.path.exists(pd):
            project_data(open(pd, "rb").read())
    media = os.path.join(band, "Media")
    if os.path.isdir(media):
        print("-- Media")
        for root, _, names in os.walk(media):
            for n in sorted(names):
                print("  ", os.path.relpath(os.path.join(root, n), band))


def project_data(data: bytes) -> None:
    hdr = read_header(data)
    chunks, end = walk(data)
    print("  ProjectData: %d bytes, format 0x%04x, fields %s, length %s, %d chunks, walk %s"
          % (len(data), hdr["format"], hdr["fields"], "ok" if hdr["length_ok"] else "WRONG",
             len(chunks), "complete" if end == len(data) else "STOPPED at %d" % end))
    counts = collections.Counter(c.tag for c in chunks)
    print("  chunks:", ", ".join("%s %d" % kv for kv in counts.most_common()))
    song = next((c for c in chunks if c.tag == "Song"), None)
    if song and len(song.body) >= 118:
        a, b = struct.unpack_from("<II", song.body, 110)
        print("  Song: u32 at +110, +114 / 10000 = %.6g, %.6g" % (a / 10000, b / 10000))
    for c in chunks:
        if c.tag == "MSeq":
            name = mseq_name(c.body)
            print("  MSeq index %-4d %s" % (c.header()["index"], "(none)" if name is None else name))
        elif c.tag == "Envi":
            names = strings(c.body)
            if names:
                print("  Envi index %-4d %s" % (c.header()["index"], names[0]))
        elif c.tag == "EvSq" and len(c.body) > EVENT:
            ev = events(c.body)
            shown = ev[:8] + (["... %d more" % (len(ev) - 8)] if len(ev) > 8 else [])
            print("  EvSq kind %d index %d: %s" % (c.header()["kind"], c.header()["index"], "; ".join(shown)))


def project_data_path(band: str) -> str:
    """A package's first alternative's ProjectData, or the path itself if it is a file."""
    if not os.path.isdir(band):
        return band
    alts = alternatives(band)
    if not alts:
        sys.exit("%s: no Alternatives/NNN/ProjectData" % band)
    return os.path.join(alts[0], "ProjectData")


def project_data_of(band: str) -> bytes:
    return open(project_data_path(band), "rb").read()


# ---------------------------------------------------------------- the link graph (§11)
#
# Offsets are rec + (from a record's first byte, header included) or event +.
# A reference names its target by the target's u32 at rec +0x08 (its index),
# or by that u32 >> 16 (a *slot*: Envi 0xa4 is index 0xa40000).

TRACK_ROW_END = 0x7FFFFFFF  # rec +0x12 of the empty Trak that closes a sequence's rows
PLACEMENT = 80  # a region's place in a folder's EvSq: 0x20 MIDI, 0x24 audio
ARRANGE = (23, 1 << 18)  # (class, index) of the arrange folder in all 31 projects of §11
AUTOMATION = (23, 2 << 18)  # the folder of automation sequences
ROW_HEIGHT = 0x3C  # the arrange MSeq's height is this times its rows


def u16(b: bytes, o: int) -> int:
    return struct.unpack_from("<H", b, o)[0]


def u32(b: bytes, o: int) -> int:
    return struct.unpack_from("<I", b, o)[0]


def rec(c: Chunk) -> bytes:
    """The record as rec + counts it: the tag reversed again, the fields, the length, the body."""
    return c.tag[::-1].encode("latin-1") + c.fields + struct.pack("<Q", len(c.body)) + c.body


def after_name(r: bytes, at: int) -> int:
    """Where the fields after a u16-length name at rec + `at` start: the name's end, made even."""
    end = at + 2 + u16(r, at)
    return end + (end & 1)


def placements(body: bytes) -> list[bytes]:
    """The 80-byte 0x20 and 0x24 events of a folder's EvSq."""
    out, i = [], 0
    while i + EVENT <= len(body):
        n = 1
        while i + (n + 1) * EVENT <= len(body) and body[i + n * EVENT + 7] & 0x80:
            n += 1
        if body[i] in (0x20, 0x24) and n * EVENT == PLACEMENT:
            out.append(body[i:i + PLACEMENT])
        i += n * EVENT
    return out


class Sequence:
    """An MSeq, the Trak rows after it, the empty Trak that ends them, and its EvSq."""

    def __init__(self, mseq: Chunk, rows: list[Chunk], evsq: Chunk):
        self.mseq, self.rows, self.evsq = mseq, rows, evsq
        self.key = (mseq.header()["kind"], u32(mseq.fields, 4))
        self.name = mseq_name(mseq.body) or ""

    def label(self) -> str:
        return "MSeq %d/%d %r" % (self.key[0], self.key[1] >> 18, self.name)


def links(data: bytes, meta: dict | None = None) -> tuple[list[str], list[str]]:
    """How the records point at one another, as §11 decoded it, and every check that failed."""
    chunks = walk(data)[0]
    out: list[str] = []
    held: collections.Counter = collections.Counter()
    fails: list[str] = []
    notes: list[str] = []

    def check(ok: bool, what: str, why: str = "") -> bool:
        if ok:
            held[what] += 1
        else:
            fails.append(what + (": " + why if why else ""))
        return ok

    # Sequences: MSeq, rows, end, EvSq, all of one class and index.
    seqs: dict[tuple[int, int], Sequence] = {}
    for i, c in enumerate(chunks):
        if c.tag != "MSeq":
            continue
        j = i + 1
        while j < len(chunks) and chunks[j].tag == "Trak" and u32(chunks[j].fields, 14) != TRACK_ROW_END:
            j += 1
        if not check(j + 1 < len(chunks) and chunks[j].tag == "Trak" and not chunks[j].body
                     and chunks[j + 1].tag == "EvSq", "MSeq, rows, end Trak, EvSq",
                     "MSeq at 0x%x" % c.offset):
            continue
        s = Sequence(c, chunks[i + 1:j], chunks[j + 1])
        check(s.evsq.fields[2:8] == c.fields[2:8], "EvSq has its MSeq's class and index", s.label())
        check(u32(c.body, 8) == u32(rec(s.evsq), 0x0E), "MSeq rec +0x2C = EvSq rec +0x0E (counter)", s.label())
        for k, r in enumerate(s.rows):
            check(u32(r.fields, 14) == k, "Trak row rec +0x12 = its place", "%s row %d" % (s.label(), k))
        seqs[s.key] = s
    counters = [u32(s.mseq.body, 8) for s in seqs.values()]
    check(len(set(counters)) == len(counters), "counters distinct")
    out.append("  sequences: %d; counters %s" % (len(seqs), " ".join("%d" % v for v in sorted(counters))))

    envi = {u32(c.fields, 4): c for c in chunks if c.tag == "Envi"}
    aufl = {u32(c.fields, 4): c for c in chunks if c.tag == "AuFl"}
    aurg = {(u32(c.fields, 4), u32(c.fields, 10)): c for c in chunks if c.tag == "AuRg"}
    # The strips: the AuCO records of the AuCn that holds the most, by their number at rec +0x0E.
    groups = collections.Counter(u32(c.fields, 4) for c in chunks if c.tag == "AuCO")
    strip_group = groups.most_common(1)[0][0] if groups else None
    strips = {u16(c.fields, 10): c for c in chunks if c.tag == "AuCO" and u32(c.fields, 4) == strip_group}
    last = None
    for c in chunks:
        if c.tag == "AuCO":
            last = c
        elif c.tag == "AuCU":
            check(last is not None and c.fields[4:12] == last.fields[4:12],
                  "AuCU rec +0x08..+0x0F = the AuCO before it", "AuCU at 0x%x" % c.offset)

    def channel(index: int, strip: bool) -> str:
        """An Envi and, if `strip`, the AuCO it must name (a track's or the master's)."""
        e = envi.get(index)
        if e is None:
            return "Envi 0x%x (none)" % (index >> 16)
        r = rec(e)
        name = (strings(e.body)[:1] or [""])[0]
        at = after_name(r, 0xC2)
        n = u16(r, at) - 1  # the strip's number, plus one
        s = strips.get(n)
        text = "Envi 0x%x %r" % (index >> 16, name)
        if s is None and not strip:
            return text  # an automation row's Envi may be no strip (the MIDI contexts)
        if check(s is not None, "Envi -> AuCO by number", text):
            text += ", strip %d %r" % (n, rec(s)[0x60:0x70].split(b"\0")[0].decode("latin-1").strip())
            key = r[at + 0x11F:at + 0x12F]
            if len(key) == 16 and any(key) and b"\xff" + key in rec(s):
                held["Envi key at +0x11F after the name = the AuCO's"] += 1
        return text

    def file_name(c: Chunk) -> str:
        return c.body[10:10 + 2 * u16(c.body, 8)].decode("utf-16-le", "replace")

    for fkey, title in ((ARRANGE, "arrange"), (AUTOMATION, "automation folder")):
        f = seqs.get(fkey)
        if not check(f is not None, "the %s at class 23, index %d" % (title, fkey[1] >> 18)):
            continue
        r = rec(f.mseq)
        line = "  %s: %s, %d rows" % (title, f.label(), len(f.rows))
        if fkey == ARRANGE:
            height = u16(r, after_name(r, 0x34) + 0x0A)
            check(height == ROW_HEIGHT * len(f.rows), "arrange height = 0x3C x rows",
                  "%d for %d rows" % (height, len(f.rows)))
            line += ", height %d = 0x3C x %d" % (height, height // ROW_HEIGHT)
        out.append(line)
        by_row = collections.defaultdict(list)
        for e in placements(f.evsq.body):
            row = u16(e, 0x14)
            if check(1 <= row <= len(f.rows), "event +0x14 = a row, from 1", "%s row %d" % (title, row)):
                by_row[row - 1].append(e)
        for k, row in enumerate(f.rows):
            b = row.body
            kind, flags, index = u16(b, 0), u16(b, 2), u32(b, 6)
            check(u16(b, 8) == index >> 16, "Trak body +0x08 = slot of body +0x06")
            check(bool(flags & 0x14) == bool(by_row[k]), "Trak body +0x02 has 0x14 iff the row has events",
                  "%s row %d" % (title, k))
            if index not in envi:
                if fkey == AUTOMATION:  # the templates keep rows of strips Apple deleted
                    notes.append("%s row %d: Envi 0x%x is not in the file" % (title, k, index >> 16))
                    continue
                check(False, "Trak row -> Envi", "%s row %d: 0x%x" % (title, k, index >> 16))
            if fkey == AUTOMATION and not by_row[k]:
                continue
            what = {1: "track", 3: "master", 4: "Drummer"}.get(kind, "kind %d" % kind)
            sel = "  selected (body +0x28 = 0x20)" if len(b) > 0x28 and b[0x28] & 0x20 else ""
            out.append("    %-8s row %-2d %s%s" % (what if fkey == ARRANGE else "", k,
                                                  channel(index, fkey == ARRANGE), sel))
            for e in by_row[k]:
                check(u16(e, 0x10) == index >> 16, "event +0x10 = its row's slot")
                at = position(u32(e, 4), REGION_BAR_ONE)
                if e[0] == 0x20:
                    s = seqs.get((23, u32(e, 0x20) << 16))
                    if not check(s is not None, "0x20 event +0x20 -> MSeq", "0x%x" % u32(e, 0x20)):
                        continue
                    sr = rec(s.mseq)
                    start = after_name(sr, 0x34)
                    check(u32(sr, start + 0xCA) == index, "region MSeq +0xCA after the name = its track's Envi",
                          s.label())
                    notes_n = sum(1 for x in events(s.evsq.body) if x.startswith("note"))
                    out.append("        %s %s -> %s, %d notes, length %d" % (
                        at, "MIDI " if fkey == ARRANGE else "0x20 ", s.label(), notes_n, u32(sr, start + 0x3C)))
                else:
                    fi, ri = u32(e, 0x2C) << 16, u32(e, 0x28)
                    fl, rg = aufl.get(fi), aurg.get((fi, ri))
                    if check(fl is not None and rg is not None, "0x24 event +0x2C, +0x28 -> AuFl, AuRg",
                             "file %d region %d" % (fi >> 18, ri)):
                        out.append("        %s audio -> AuFl %d %r, AuRg %d/%d %r" % (
                            at, fi >> 18, file_name(fl), fi >> 18, ri, (strings(rg.body)[:1] or [""])[0]))
        if fkey == AUTOMATION:
            out.append("    (and %d rows with no events)" % sum(1 for k in range(len(f.rows)) if not by_row[k]))
    for (fi, ri), rg in sorted(aurg.items()):
        check(fi in aufl, "AuRg -> the AuFl of its index", "AuRg %d/%d" % (fi >> 18, ri))

    arrange = seqs.get(ARRANGE)
    if arrange:
        tracks = [r for r in arrange.rows if u16(r.body, 0) != 3]
        if meta and "NumberOfTracks" in meta:
            check(meta["NumberOfTracks"] == len(tracks), "MetaData NumberOfTracks = rows - master")
        song = next((c for c in chunks if c.tag == "Song"), None)
        if song:
            r = rec(song)
            slot, number = u16(r, 0x82), u16(r, 0xF6)
            selected = [k for k, t in enumerate(arrange.rows) if len(t.body) > 0x28 and t.body[0x28] & 0x20]
            out.append("  Song: rec +0x82 slot 0x%x, rec +0xF6 track %d; rows marked selected: %s"
                       % (slot, number, [k + 1 for k in selected] or "none"))
            if selected:
                check(u16(arrange.rows[selected[0]].body, 8) == slot and number == selected[0] + 1,
                      "Song +0x82, +0xF6 = the selected row")
            else:
                check(slot == 0 and number == 0, "Song +0x82, +0xF6 = 0 when no row is selected")
            # The registry: 24-byte rows [u32 class][u32 slot][a version-1 UUID], for Envi (20) and class 23.
            reg = collections.defaultdict(set)
            for k in range(0, len(r) - 24, 4):
                cls, s = u32(r, k), u32(r, k + 4)
                if cls in (20, 23) and s < 0x10000 and not s & 3 and r[k + 14:k + 16] == b"\x11\xf1":
                    reg[cls].add(s)
            if reg:
                have = {20: {i >> 16 for i in envi}, 23: {i >> 16 for (c, i) in seqs if c == 23}}
                for cls in (20, 23):
                    check(reg[cls] == have[cls], "Song registry = every %s" % ("Envi" if cls == 20 else "class-23 MSeq"),
                          "%d rows for %d" % (len(reg[cls]), len(have[cls])))
                out.append("  Song registry: %d Envi rows, %d class-23 rows" % (len(reg[20]), len(reg[23])))
            else:
                out.append("  Song registry: none (an older Song)")
    out.append("  checks held:")
    out.extend("    %4d  %s" % (n, what) for what, n in sorted(held.items()))
    for n in notes:
        out.append("  note: " + n)
    for f in fails:
        out.append("  FAILED: " + f)
    if not fails:
        out.append("  every check held")
    return out, fails



def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("bands", nargs="+")
    ap.add_argument("--chunks", action="store_true", help="list every chunk's header")
    ap.add_argument("--dump", metavar="TAG", help="hex dump every chunk with this tag")
    ap.add_argument("--diff", action="store_true", help="compare the chunks of two projects")
    ap.add_argument("--links", action="store_true", help="print the link graph and check it")
    a = ap.parse_args()
    if a.diff:
        if len(a.bands) != 2:
            sys.exit("--diff takes two projects")
        diff(*a.bands)
        return
    if a.links:
        failed = 0
        for band in a.bands:
            print("=" * 72)
            print(band)
            meta = plist(os.path.join(alternatives(band)[0], "MetaData.plist")) if os.path.isdir(band) else None
            lines, fails = links(open(project_data_path(band), "rb").read(), meta)
            print("\n".join(lines))
            failed += bool(fails)
        sys.exit(1 if failed else 0)
    for band in a.bands:
        if a.chunks or a.dump:
            for c in walk(project_data_of(band))[0]:
                if a.dump and c.tag != a.dump:
                    continue
                h = c.header()
                print("0x%07x %s v%d kind %d index %d links %s tail %s len %d"
                      % (c.offset, c.tag, h["version"], h["kind"], h["index"], h["links"], h["tail"], len(c.body)))
                if a.dump:
                    for i in range(0, len(c.body), 16):
                        print("    %04x  %s" % (i, c.body[i:i + 16].hex(" ")))
        else:
            summarize(band)


def diff(a: str, b: str) -> None:
    """Pair the chunks of two projects by tag, kind and index (then by order), and say which differ."""
    def keyed(band):
        out = collections.OrderedDict()
        for c in walk(project_data_of(band))[0]:
            h = c.header()
            n = 0
            while (c.tag, h["kind"], h["index"], n) in out:
                n += 1
            k = (c.tag, h["kind"], h["index"], n)
            out[k] = c
        return out
    ka, kb = keyed(a), keyed(b)
    for k in list(ka) + [k for k in kb if k not in ka]:
        ca, cb = ka.get(k), kb.get(k)
        label = "%s kind %d index %d #%d" % k
        if ca is None:
            print("+ %s (%d bytes)  %s" % (label, len(cb.body), strings(cb.body)[:2]))
        elif cb is None:
            print("- %s (%d bytes)  %s" % (label, len(ca.body), strings(ca.body)[:2]))
        elif ca.body != cb.body:
            n = min(len(ca.body), len(cb.body))
            at = [i for i in range(n) if ca.body[i] != cb.body[i]]
            print("~ %s %d -> %d bytes, %d bytes differ, first at %s  %s"
                  % (label, len(ca.body), len(cb.body), len(at) + abs(len(ca.body) - len(cb.body)),
                     at[:6], strings(cb.body)[:2]))


if __name__ == "__main__":
    try:
        main()
        sys.stdout.flush()
    except BrokenPipeError:  # piped into head: stop quietly
        os.dup2(os.open(os.devnull, os.O_WRONLY), sys.stdout.fileno())
        sys.exit(1)
