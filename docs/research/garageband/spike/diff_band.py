#!/usr/bin/env python3
"""Compare two GarageBand projects record by record (docs/research/garageband §10).

    python3 -I diff_band.py A.band B.band              # what changed, ids folded
    python3 -I diff_band.py --ids A.band B.band        # and every id that changed, with where else B holds it
    python3 -I diff_band.py --tag Trak A.band B.band   # only one tag (repeatable)
    python3 -I diff_band.py --summary A.band B.band    # counts only

A and B are `.band` packages or bare `ProjectData` files. Reads only; standard
library only; it imports `inspect_band.py` from beside it.

Pairing. The records are paired by position: their run of (tag, class,
index, length, name) is aligned as a diff aligns lines. Records left over in a
changed stretch are paired again by (tag, class, index, name), then by (tag,
class, index). A run of added records beside records that look the same (a
new channel strip's AuCU next to an old one's) is slid to where the most bytes
stay equal. So an added or removed record shows where it sits. A record whose
bytes are another record's is reported as moved (`>`), or as re-sorted when
two records at one index swapped places: a diff keyed by index would show
those as changed bodies (LogicProFormatWriter's spec §10.6.6).

Bytes. For each pair it prints the runs of bytes that differ at `rec +` (from
the record's first byte, header included, as LogicProFormatWriter counts) and
`body +` (rec + 0x24). Bodies of one length are compared in place; others are
aligned byte by byte, or by u32 for long bodies such as the Song. An event
sequence (EvSq) is compared by 16-byte rows, so an inserted event shows as
rows. Three kinds of noise are folded:

- an *id*: 16 bytes whose bytes 0-3 and 8-15 changed and whose bytes 4-7 did
  not. GarageBand writes new ones each time it opens a MIDI file, and keeps
  them while one document stays open. `--ids` lists them;
- 4 or more u32s changed at one stride, as the Song's rows of time stamps;
- 3 or more records in a row whose small numbers changed alike, as when an
  inserted strip renumbers every AuCO after it.
"""

from __future__ import annotations

import argparse
import bisect
import collections
import difflib
import hashlib
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # inspect_band beside it; -I leaves it out
import inspect_band as ib  # noqa: E402

BODY = ib.CHUNK_HEADER  # body + 0 is rec + 0x24
FIELDS = 4  # Chunk.fields starts at rec + 0x04
ID = 16
SLOW = 1024  # middles of different lengths up to this are aligned byte by byte


# ---------------------------------------------------------------- loading

class Song:
    def __init__(self, path: str):
        self.path = path
        if os.path.isdir(path):
            alts = ib.alternatives(path)
            if not alts:
                sys.exit("%s: no Alternatives/NNN/ProjectData" % path)
            self.alt = alts[0]
            self.data = open(os.path.join(self.alt, "ProjectData"), "rb").read()
        else:
            self.alt, self.data = None, open(path, "rb").read()
        self.header = ib.read_header(self.data)
        self.recs, end = ib.walk(self.data)
        if end != len(self.data):
            print("! %s: the walk stopped at %d of %d" % (path, end, len(self.data)))

    def files(self) -> dict[str, int]:
        return dict(ib.files(self.path)) if os.path.isdir(self.path) else {}

    def meta(self) -> dict:
        return (ib.plist(os.path.join(self.alt, "MetaData.plist")) or {}) if self.alt else {}


def raw(c: ib.Chunk) -> bytes:
    """The record's own bytes from rec + 0x04 on (tag and length left out)."""
    return c.fields + c.body


def digest(c: ib.Chunk) -> tuple:
    return (c.tag, hashlib.sha1(raw(c)).digest())


def key(c: ib.Chunk) -> tuple:
    h = c.header()
    return (c.tag, h["kind"], struct.unpack_from("<I", c.fields, 4)[0])


def name(c: ib.Chunk) -> str:
    if c.tag == "AuFl" and len(c.body) > 10:
        n = struct.unpack_from("<H", c.body, 8)[0]
        return c.body[10:10 + 2 * n].decode("utf-16-le", "replace")
    if c.tag in ("MSeq", "Envi", "AuRg", "Layr"):
        return (ib.strings(c.body)[:1] or [""])[0]
    return ""


def label(c: ib.Chunk, i: int) -> str:
    tag, kind, index = key(c)
    n = name(c)
    return "#%d %s class %d index 0x%x (%d) %dB%s" % (
        i, tag, kind, index, index >> 18, len(c.body), "  %r" % n if n else "")


# ---------------------------------------------------------------- pairing

SLIDE = 64  # how far a run of added records may slide along a run of look-alikes


def pair(a: list[ib.Chunk], b: list[ib.Chunk]) -> list[tuple[int | None, int | None]]:
    """(i, j) in B's order: i or j is None for a record only in B or only in A.

    The records' (tag, class, index, length, name) are aligned as lines are.
    Records left over in a changed stretch are paired by (tag, class, index,
    name), then by (tag, class, index). A run of added (or removed) records
    next to records that look the same, such as a new channel strip's AuCU
    beside an old one's, is then slid to where the most bytes stay equal.
    """
    def tok(c):
        return key(c) + (len(c.body), name(c))
    ta, tb = [tok(c) for c in a], [tok(c) for c in b]
    pa: list[int | None] = [None] * len(b)
    used = set()
    for op, i1, i2, j1, j2 in difflib.SequenceMatcher(None, ta, tb, autojunk=False).get_opcodes():
        if op == "equal":
            for i, j in zip(range(i1, i2), range(j1, j2)):
                pa[j] = i
                used.add(i)
            continue
        for match in (lambda c: key(c) + (name(c),), key):
            left = collections.defaultdict(collections.deque)
            for i in range(i1, i2):
                if i not in used:
                    left[match(a[i])].append(i)
            for j in range(j1, j2):
                q = left.get(match(b[j])) if pa[j] is None else None
                if q:
                    pa[j] = q.popleft()
                    used.add(pa[j])
    pa = slide(pa, a, b, tb)
    pb: list[int | None] = [None] * len(a)
    for j, i in enumerate(pa):
        if i is not None:
            pb[i] = j
    pb = slide(pb, b, a, ta)
    pa = [None] * len(b)
    for i, j in enumerate(pb):
        if j is not None:
            pa[j] = i
    # B's order; a record only in A goes before the first B record paired after it
    gone = sorted(i for i in range(len(a)) if pb[i] is None)
    out, g = [], 0
    for j, i in enumerate(pa):
        while i is not None and g < len(gone) and gone[g] < i:
            out.append((gone[g], None))
            g += 1
        out.append((i, j))
    return out + [(i, None) for i in gone[g:]]


def alike(x: ib.Chunk, y: ib.Chunk) -> int:
    """How many bytes two records share in place (0 if their lengths differ)."""
    if len(x.body) != len(y.body):
        return 0
    return sum(p == q for p, q in zip(raw(x), raw(y)))


def slide(pa: list, a: list, b: list, tb: list) -> list:
    """Move each run of records only in b along the paired records beside it, if that pairs better."""
    pa = list(pa)
    j = 0
    while j < len(b):
        if pa[j] is not None:
            j += 1
            continue
        j2 = j
        while j2 < len(b) and pa[j2] is None:
            j2 += 1
        best, shift, gain = 0, 0, 0
        for t in range(1, SLIDE + 1):  # slide left: b[j-t] becomes added, b[j2-t] takes its partner
            if j - t < 0 or pa[j - t] is None or tb[j - t] != tb[j2 - t] or pa[j - t] != pa[j - 1] - (t - 1):
                break
            gain += alike(a[pa[j - t]], b[j2 - t]) - alike(a[pa[j - t]], b[j - t])
            if gain > best:
                best, shift = gain, -t
        gain = 0
        for t in range(SLIDE):  # slide right: b[j2+t] becomes added, b[j+t] takes its partner
            if j2 + t >= len(b) or pa[j2 + t] is None or tb[j + t] != tb[j2 + t] or pa[j2 + t] != pa[j2] + t:
                break
            gain += alike(a[pa[j2 + t]], b[j + t]) - alike(a[pa[j2 + t]], b[j2 + t])
            if gain > best:
                best, shift = gain, t + 1
        n = j2 - j
        if shift < 0:
            s = -shift
            pa[j - s:j2] = [None] * n + pa[j - s:j]
            j = j2
        elif shift > 0:
            pa[j:j2 + shift] = pa[j2:j2 + shift] + [None] * n
            j = j2 + shift
        else:
            j = j2
    return pa


# ---------------------------------------------------------------- bytes

def in_place(x: bytes, y: bytes, i0: int = 0, j0: int = 0) -> list[tuple[int, bytes, int, bytes]]:
    """The stretches that differ between two strings of one length, compared byte for byte."""
    out, i = [], 0
    while i < len(x):
        if x[i] != y[i]:
            k = i
            while k < len(x) and x[k] != y[k]:
                k += 1
            out.append((i0 + i, x[i:k], j0 + i, y[i:k]))
            i = k
        else:
            i += 1
    return out


def runs(x: bytes, y: bytes) -> list[tuple[int, bytes, int, bytes]]:
    """(offset in x, bytes of x, offset in y, bytes of y) for every stretch that differs.

    Strings of one length are compared in place. Otherwise the common head and
    tail are set aside. A short middle is aligned byte by byte; a long one
    (the Song, whose ids all change) is taken as one insertion or deletion at
    the split that leaves the most bytes equal, the rest compared in place.
    """
    if len(x) == len(y):
        return in_place(x, y)
    p = 0
    while p < min(len(x), len(y)) and x[p] == y[p]:
        p += 1
    s = 0
    while s < min(len(x), len(y)) - p and x[-1 - s] == y[-1 - s]:
        s += 1
    mx, my = x[p:len(x) - s], y[p:len(y) - s]
    if len(mx) + len(my) <= SLOW:
        sm = difflib.SequenceMatcher(None, mx, my, autojunk=False)
        return [(p + i1, mx[i1:i2], p + j1, my[j1:j2]) for op, i1, i2, j1, j2 in sm.get_opcodes() if op != "equal"]
    if (len(y) - len(x)) % 4 == 0:  # the Song's tables have rows of 16 and 24 bytes: align u32s
        wx = [x[k:k + 4] for k in range(0, len(x), 4)]
        wy = [y[k:k + 4] for k in range(0, len(y), 4)]
        out = []
        for op, i1, i2, j1, j2 in difflib.SequenceMatcher(None, wx, wy, autojunk=False).get_opcodes():
            if op == "equal":
                continue
            bx, by = x[4 * i1:4 * i2], y[4 * j1:4 * j2]
            out += in_place(bx, by, 4 * i1, 4 * j1) if len(bx) == len(by) else split(bx, by, 4 * i1, 4 * j1)
        return out
    return split(mx, my, p, p)


def split(mx: bytes, my: bytes, i0: int, j0: int) -> list[tuple[int, bytes, int, bytes]]:
    """Two strings of different lengths as one insertion or deletion at the split that leaves
    the most bytes equal in place on either side of it."""
    if not mx or not my:
        return [(i0, mx, j0, my)]
    d = len(my) - len(mx)
    short, long_ = (mx, my) if d > 0 else (my, mx)
    d = abs(d)
    # equal bytes before q in place, and from q on with the long string d bytes ahead
    head = [0]
    for k in range(len(short)):
        head.append(head[-1] + (short[k] == long_[k]))
    tail = [0] * (len(short) + 1)
    for k in range(len(short) - 1, -1, -1):
        tail[k] = tail[k + 1] + (short[k] == long_[k + d])
    q = max(range(len(short) + 1), key=lambda k: head[k] + tail[k])
    if len(mx) < len(my):
        return (in_place(mx[:q], my[:q], i0, j0) + [(i0 + q, b"", j0 + q, my[q:q + d])]
                + in_place(mx[q:], my[q + d:], i0 + q, j0 + q + d))
    return (in_place(mx[:q], my[:q], i0, j0) + [(i0 + q, mx[q:q + d], j0 + q, b"")]
            + in_place(mx[q + d:], my[q:], i0 + q + d, j0 + q))


def fold_ids(x: bytes, y: bytes, rs: list) -> tuple[list, list]:
    """Split the runs into ids (see the module's doc) and the rest.

    An id is 16 bytes, at i in x and j in y, whose bytes 4-7 are equal and of
    which 8 or more differ. The runs inside it are dropped, and a run that
    goes on past its end is cut there. The ids are (j, x's 16 bytes, y's).
    """
    rs = list(rs)
    ids, rest, k = [], [], 0
    while k < len(rs):
        i, bx, j, by = rs[k]
        if (len(bx) == len(by) and 1 <= len(bx) <= 4 and i + ID <= len(x) and j + ID <= len(y)
                and x[i + 4:i + 8] == y[j + 4:j + 8]
                and sum(x[i + t] != y[j + t] for t in range(ID)) >= 8):
            n = k + 1
            while n < len(rs) and rs[n][2] - rs[n][0] == j - i and rs[n][0] < i + ID \
                    and len(rs[n][1]) == len(rs[n][3]):
                n += 1
            covered = {p for r in rs[k:n] for p in range(r[0], r[0] + len(r[1]))}
            if any(x[i + t] != y[j + t] and i + t not in covered for t in range(ID)):
                rest.append(rs[k])  # shifted text, not an id: the bytes differ outside these runs
                k += 1
                continue
            ids.append((j, x[i:i + ID], y[j:j + ID]))
            ri, rbx, rj, rby = rs[n - 1]
            cut = i + ID - ri
            if n - 1 > k and cut < len(rbx):
                rs.insert(n, (i + ID, rbx[cut:], j + ID, rby[cut:]))
            k = n
            continue
        rest.append(rs[k])
        k += 1
    return ids, rest


def show(bx: bytes, by: bytes) -> str:
    def h(b):
        return b.hex() if len(b) <= 24 else "%s…(%d)" % (b[:24].hex(), len(b))
    s = "%s -> %s" % (h(bx) or "''", h(by) or "''")
    if len(bx) == len(by) and len(bx) in (1, 2, 4):
        f = {1: "<B", 2: "<H", 4: "<I"}[len(bx)]
        s += "  (%d -> %d)" % (struct.unpack(f, bx)[0], struct.unpack(f, by)[0])
    return s


def where(rec: int) -> str:
    """An offset from the record's first byte, as rec + and, past the header, body +."""
    return "rec +0x%02x" % rec if rec < BODY else "rec +0x%02x (body +0x%02x)" % (rec, rec - BODY)


def evsq_rows(x: bytes, y: bytes) -> list[str]:
    """An event sequence's body compared as 16-byte rows."""
    rx = [x[i:i + 16] for i in range(0, len(x), 16)]
    ry = [y[i:i + 16] for i in range(0, len(y), 16)]
    out = []
    sm = difflib.SequenceMatcher(None, rx, ry, autojunk=False)
    for op, i1, i2, j1, j2 in sm.get_opcodes():
        if op == "equal":
            continue
        if op == "replace" and i2 - i1 == j2 - j1:
            for i, j in zip(range(i1, i2), range(j1, j2)):
                d = [p for p in range(16) if rx[i][p] != ry[j][p]]
                out.append("    row %d (body +0x%x) bytes %s: %s -> %s"
                           % (j, 16 * j, d, rx[i].hex(" "), ry[j].hex(" ")))
            continue
        for i in range(i1, i2):
            out.append("    - row %d (body +0x%x) %s" % (i, 16 * i, rx[i].hex(" ")))
        for j in range(j1, j2):
            out.append("    + row %d (body +0x%x) %s" % (j, 16 * j, ry[j].hex(" ")))
    return out


# ---------------------------------------------------------------- report

def compare(ca: ib.Chunk, cb: ib.Chunk) -> tuple[list[str], list, tuple | None]:
    """Lines for what differs (ids folded), the ids that changed, and a signature: see signature()."""
    head = runs(ca.fields, cb.fields)
    lines = ["    %s: %s" % (where(FIELDS + i), show(bx, by)) for i, bx, j, by in head]
    rs = runs(ca.body, cb.body)
    ids, rest = fold_ids(ca.body, cb.body, rs)
    sig = signature(ca, cb, head, rs) if not ids else None
    if ca.tag == "EvSq" and len(ca.body) % 16 == 0 and len(cb.body) % 16 == 0:
        if rest:
            lines += evsq_rows(ca.body, cb.body)
        return lines, [(BODY + i, a, b) for i, a, b in ids], sig
    for item in strided(ca.body, cb.body, rest):
        if isinstance(item, str):
            lines.append(item)
            continue
        i, bx, j, by = item
        at = where(BODY + i)
        if i != j:
            at += " / B %s" % where(BODY + j)
        lines.append("    %s: %s" % (at, show(bx, by)))
    return lines, [(BODY + i, a, b) for i, a, b in ids], sig


def signature(ca: ib.Chunk, cb: ib.Chunk, head: list, body: list) -> tuple | None:
    """(tag, ((rec offset, size, delta), ...)) when every change is a small number changed in place."""
    if len(ca.body) != len(cb.body) or not head + body:
        return None
    x, y = ca.fields + bytes(8) + ca.body, cb.fields + bytes(8) + cb.body  # from rec + 0x04
    fields = []
    for base, rs in ((FIELDS, head), (FIELDS, [(i + BODY - FIELDS, bx, j, by) for i, bx, j, by in body])):
        for i, bx, j, by in rs:
            rec = base + i
            for size in (2, 4):
                w = rec - rec % size
                if rec + len(bx) <= w + size:
                    break
            else:
                return None
            o = w - FIELDS
            fields.append((w, size, int.from_bytes(y[o:o + size], "little") - int.from_bytes(x[o:o + size], "little")))
    return (ca.tag, tuple(dict.fromkeys(fields)))


def strided(x: bytes, y: bytes, rs: list) -> list:
    """Fold 4 or more changed u32s at a fixed stride (the Song's rows of time stamps) into one line."""
    def word(r):
        i, bx, j, by = r
        w = i - i % 4
        return (w, j - i) if len(bx) == len(by) and i + len(bx) <= w + 4 else (None, None)
    out, k = [], 0
    while k < len(rs):
        w0, d0 = word(rs[k])
        n = k + 1
        if w0 is not None and n < len(rs) and word(rs[n])[1] == d0:
            step = word(rs[n])[0] - w0
            while n < len(rs) and word(rs[n]) == (w0 + step * (n - k), d0):
                n += 1
        if w0 is None or n - k < 4 or step <= 0:
            out.append(rs[k])
            k += 1
            continue
        last = word(rs[n - 1])[0]
        out.append("    %d u32s every 0x%x from %s to body +0x%x%s: first %s, last %s"
                   % (n - k, step, where(BODY + w0), last, " (B %+d)" % d0 if d0 else "",
                      show(x[w0:w0 + 4], y[w0 + d0:w0 + d0 + 4]),
                      show(x[last:last + 4], y[last + d0:last + d0 + 4])))
        k = n
    return out


def where_else(song: Song, value: bytes, skip: int) -> list[str]:
    """Which records of the song hold these bytes, other than the one at offset skip."""
    out, start = [], 0
    starts = [c.offset for c in song.recs]
    while True:
        p = song.data.find(value, start)
        if p < 0:
            return out
        start = p + 1
        k = bisect.bisect_right(starts, p) - 1
        c = song.recs[k]
        if c.offset == skip:
            continue
        out.append("#%d %s %s" % (k, c.tag, "rec +0x%x" % (p - c.offset)))


def renumbered(out: list) -> list[str]:
    """The report's lines, with 3 or more records in a row that changed alike folded into one."""
    lines, k = [], 0
    while k < len(out):
        sig = out[k][0]
        n = k + 1
        while sig and n < len(out) and out[n][0] == sig:
            n += 1
        if not sig or n - k < 3:
            lines += out[k][1]
            k += 1
            continue
        tag, fields = sig
        first, last = out[k], out[n - 1]

        def val(c, off, size):
            raw_ = c.fields + bytes(8) + c.body
            return int.from_bytes(raw_[off - FIELDS:off - FIELDS + size], "little")
        lines.append("~ %d %s records #%d to #%d changed alike: %s"
                     % (n - k, tag, first[2], last[2], "; ".join(
                         "u%d at %s %+d (%d -> %d ... %d -> %d)"
                         % (8 * size, where(off), d, val(first[3], off, size), val(first[4], off, size),
                            val(last[3], off, size), val(last[4], off, size))
                         for off, size, d in fields)))
        k = n
    return lines


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("a")
    ap.add_argument("b")
    ap.add_argument("--tag", action="append", help="only records with this tag (repeatable)")
    ap.add_argument("--ids", action="store_true", help="list the ids that changed, and where else B holds each")
    ap.add_argument("--summary", action="store_true", help="counts only")
    o = ap.parse_args()
    A, B = Song(o.a), Song(o.b)

    print("A %s: %d bytes, format 0x%04x, %d records" % (o.a, len(A.data), A.header["format"], len(A.recs)))
    print("B %s: %d bytes, format 0x%04x, %d records" % (o.b, len(B.data), B.header["format"], len(B.recs)))
    if A.data[:16] != B.data[:16]:
        print("file header: %s -> %s" % (A.data[4:16].hex(), B.data[4:16].hex()))
    ta = collections.Counter(c.tag for c in A.recs)
    tb = collections.Counter(c.tag for c in B.recs)
    delta = ["%s %d -> %d" % (t, ta[t], tb[t]) for t in sorted(set(ta) | set(tb)) if ta[t] != tb[t]]
    print("records by tag:", ", ".join(delta) if delta else "the same counts")
    fa, fb = A.files(), B.files()
    for f in sorted(set(fa) | set(fb)):
        if f.endswith("ProjectData") or f.endswith(".jpg") or "DisplayState" in f:
            continue
        if f not in fa:
            print("file + %s (%d)" % (f, fb[f]))
        elif f not in fb:
            print("file - %s (%d)" % (f, fa[f]))
    ma, mb = A.meta(), B.meta()
    for k in sorted(set(ma) | set(mb)):
        if ma.get(k) != mb.get(k):
            print("MetaData.%s: %r -> %r" % (k, ma.get(k), mb.get(k)))

    pairs = pair(A.recs, B.recs)
    # moves: a record only in A whose bytes are a record only in B, or a pair whose B bytes are another A record's
    only_a = {i for i, j in pairs if j is None}
    only_b = {digest(B.recs[j]) for i, j in pairs if i is None}
    by_bytes = collections.defaultdict(list)
    for i, c in enumerate(A.recs):
        by_bytes[digest(c)].append(i)
    counts = collections.Counter()
    all_ids, out = [], []
    for i, j in pairs:
        ca = A.recs[i] if i is not None else None
        cb = B.recs[j] if j is not None else None
        c = ca or cb
        if o.tag and c.tag not in o.tag:
            continue
        if ca is None:
            same = [k for k in by_bytes.get(digest(cb), []) if k in only_a]
            counts["moved" if same else "added"] += 1
            out.append((None, [("> %s  (A's #%d, moved)" % (label(cb, j), same[0])) if same
                               else "+ %s" % label(cb, j)]))
            continue
        if cb is None:
            if digest(ca) in only_b:
                continue  # reported as a move on B's side
            counts["removed"] += 1
            out.append((None, ["- %s" % label(ca, i)]))
            continue
        if raw(ca) == raw(cb):
            counts["same"] += 1
            continue
        lines, ids, sig = compare(ca, cb)
        all_ids += [(j, cb, off, x, y) for off, x, y in ids]
        if not lines:
            counts["ids only"] += 1
            continue
        counts["changed"] += 1
        twin = [k for k in by_bytes.get(digest(cb), []) if k != i]
        note = "  (= A's #%d: re-sorted)" % twin[0] if twin else ""
        entry = ["~ %s%s%s" % (label(cb, j), "" if i == j else "  (A #%d)" % i, note)]
        if len(ca.body) != len(cb.body):
            entry.append("    length %d -> %d" % (len(ca.body), len(cb.body)))
        if ids:
            entry.append("    and %d id%s" % (len(ids), "s" if len(ids) > 1 else ""))
        out.append((None if twin or ids else sig, entry + lines, j, ca, cb))

    print("records:", ", ".join("%d %s" % (n, k) for k, n in sorted(counts.items())))
    if all_ids:
        stamps = collections.Counter(y[4:8].hex() for _, _, _, _, y in all_ids)
        print("ids changed: %d, in %d records; their bytes 4-7: %s"
              % (len(all_ids), len({j for j, *_ in all_ids}),
                 ", ".join("%s ×%d" % kv for kv in stamps.most_common(4))))
    if o.summary:
        return
    for line in renumbered(out):
        print(line)
    if o.ids:
        print("-- ids that changed (A -> B), and where else B holds the new one")
        for j, cb, off, x, y in all_ids:
            print("  %s %s: %s -> %s  also at: %s"
                  % (label(cb, j), where(off), x.hex(), y.hex(),
                     ", ".join(where_else(B, y, cb.offset)) or "nowhere"))


if __name__ == "__main__":
    try:
        main()
        sys.stdout.flush()
    except BrokenPipeError:  # piped into head: stop quietly
        os.dup2(os.open(os.devnull, os.O_WRONLY), sys.stdout.fileno())
        sys.exit(1)
