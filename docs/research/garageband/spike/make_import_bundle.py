#!/usr/bin/env python3
"""Make a MIDI file and stems for GarageBand from a sample score (§4).

    python3 -I make_import_bundle.py OUT_DIR [--lpfw LPFW_DIR]
    python3 -I make_import_bundle.py --fixture DIR

Writes the sample score of §4.1 to OUT_DIR/work/, compiles it with
`lilypond`, and makes the folder OUT_DIR/Lily Import Test (GarageBand)/ that
§3.5's route would hand to the user:

- `Lily Import Test.mid`: lilypond's MIDI rewritten as the studio could (§3.3):
  each track named by its part, a program change and CC 7/10 on each track,
  the tempo events scaled to `% BPM:`, and a lead-in so the score's bar 1 is
  GarageBand's bar 2 after the pickup;
- `Song.wav`, `Drums.wav`, `Bass.wav`: the score's `% Audio:` recording and
  two stems of it, padded or cut so their sample 0 is the MIDI's tick 0;
- `probes/`: what §3.10 asks about besides: lilypond's MIDI as it was, the
  drums stem at 48 kHz and in FLAC, a small MusicXML file, and `Lean.mid`,
  the MIDI file without the words' track and the control track's texts.

With `--fixture DIR` instead of OUT_DIR, it writes the fixture of Lily
Studio's rewrite (D162) and nothing else: `garageband.ly` and lilypond's
`garageband.midi` of the sample score, `garageband.plan.json`, the plan the
studio's renderer would send, and `garageband.expected.mid`, the rewrite of
that plan as `rewrite(..., studio=True)` makes it, which the session crate's
tests compare theirs with.

With `--lpfw LPFW_DIR`, a clone of LogicProFormatWriter, it also writes
OUT_DIR/lpfw/ through LPFW's own calls (§12): `seed.logicx`, LPFW's mixed
seed with nothing added; `audio.logicx`, its beatmap export with an audio
track per stem; and `av.logicx`, an instrument track per part of `Lean.mid`
with its notes, an audio track per stem, and the tempo map and meter. LPFW's
code is imported from that clone, not copied.

There is no real recording: the spike synthesizes one from the score's tempo
map at `% BPM:`, placed by `% Audio offset:` as D155 defines it (the
recording's time is the music's time less the offset). A click on every
beat, higher on the downbeat, and a bass note on every downbeat; the two
stems are those two halves of it. Dropped at bar 1, their clicks should fall
on GarageBand's beats if the alignment is right.

Standard library only, apart from `lilypond` on the PATH and macOS's
`afconvert` for the FLAC probe.
"""

from __future__ import annotations

import math
import os
import shutil
import struct
import subprocess
import sys
import wave

TITLE = "Lily Import Test"
BPM = 113.76  # % BPM:
AUDIO_OFFSET = -2.40  # % Audio offset:, seconds
PICKUP_QUARTERS = 1  # \partial 4; the studio has it from barOneTime (D159)
BAR_QUARTERS = 3  # 3/4
RATE = 44100

SCORE = r"""\version "2.24.0"
% BPM: {bpm}
% Audio: Song (recorded).wav
% Audio offset: {offset}
\header {{ title = "{title}" }}

global = {{ \key g \major \time 3/4 \partial 4 }}

melody = \relative c'' {{
  \global \tempo 4 = 112
  d4 | g2 a4 | b2 a4 | g4 fis e | d2 d4 |
  \tempo 4 = 84
  e2 fis4 | g2 a4 | b4 a fis | g2. \bar "|."
}}

\score {{
  <<
    \new Staff = "voice" \with {{ instrumentName = "Voice" midiInstrument = "choir aahs" }}
      \new Voice = "v" \melody
    \new Lyrics \lyricsto "v" {{ \repeat unfold 18 {{ la }} }}
    \new Staff = "violin" \with {{ instrumentName = "Violin" midiInstrument = "violin" }}
      \relative c'' {{ \global b4 | d2. | d2. | b2. | a2 a4 | c2. | b2. | d2 c4 | b2. }}
    \new Staff = "cello" \with {{ instrumentName = "Cello" midiInstrument = "cello" }}
      \relative c {{ \clef bass \global r4 | g2. | g2. | e2. | d2. | c2. | g'2. | d2. | g,2. }}
    \new PianoStaff \with {{ instrumentName = "Piano" }} <<
      \new Staff = "rh" \relative c'' {{ \global r4 | <g b>2. | <g b>2. | <e g>2. | <d fis>2. | <c e>2. | <g' b>2. | <fis a>2. | <g b>2. }}
      \new Staff = "lh" \relative c {{ \clef bass \global r4 | g2. | g2. | e2. | d2. | c2. | g2. | d'2. | g,2. }}
    >>
    \new DrumStaff \with {{ instrumentName = "Drums" }} \drummode {{
      \time 3/4 \partial 4 hh4 | \repeat unfold 8 {{ bd4 sn sn }}
    }}
  >>
  \layout {{ }}
  \midi {{ }}
}}
"""

# What partNames and the Parts fold would give each of lilypond's tracks
# (§1.3), keyed by the track's name, `<staff id>:<voice id>`; the drum track
# has no name and is found by its channel. Programs are 0-based, as in the
# file. Volume and pan are CC 7 and CC 10, to see whether they arrive.
PARTS = {
    "voice:v": ("Voice", 52, 100, 64),  # choir aahs
    "violin:": ("Violin", 40, 90, 40),
    "cello:": ("Cello", 42, 90, 88),
    "rh:": ("Piano · G clef", 0, 80, 64),  # no midiInstrument in the score
    "lh:": ("Piano · F clef", 0, 80, 64),
    "drums": ("Drums", 0, 110, 64),  # channel 9
}
LYRICS_NAME = "Words"


# --- reading and writing a standard MIDI file --------------------------------

class Event:
    __slots__ = ("tick", "data")

    def __init__(self, tick: int, data: bytes):
        self.tick, self.data = tick, data  # data: status byte and all, no delta

    def meta(self) -> int | None:
        return self.data[1] if self.data[0] == 0xFF else None

    def channel(self) -> int | None:
        return self.data[0] & 0x0F if 0x80 <= self.data[0] < 0xF0 else None

    def kind(self) -> int:
        return self.data[0] & 0xF0


def read_varlen(b: bytes, i: int) -> tuple[int, int]:
    value = 0
    while True:
        c = b[i]
        i += 1
        value = (value << 7) | (c & 0x7F)
        if c < 0x80:
            return value, i


def varlen(value: int) -> bytes:
    out = [value & 0x7F]
    value >>= 7
    while value:
        out.append(0x80 | (value & 0x7F))
        value >>= 7
    return bytes(reversed(out))


def read_midi(data: bytes) -> tuple[int, int, list[list[Event]]]:
    if data[:4] != b"MThd":
        raise ValueError("not a MIDI file")
    fmt, ntracks, division = struct.unpack_from(">HHH", data, 8)
    if division & 0x8000:
        raise ValueError("SMPTE time is not handled")
    i, tracks = 8 + struct.unpack_from(">I", data, 4)[0], []
    for _ in range(ntracks):
        assert data[i:i + 4] == b"MTrk", data[i:i + 4]
        end = i + 8 + struct.unpack_from(">I", data, i + 4)[0]
        i += 8
        tick, status, events = 0, 0, []
        while i < end:
            delta, i = read_varlen(data, i)
            tick += delta
            start = i
            if data[i] >= 0x80:
                status = data[i]
                i += 1
            if status == 0xFF:
                n, i = read_varlen(data, i + 1)
                i += n
                events.append(Event(tick, data[start:i]))
                continue
            if status in (0xF0, 0xF7):
                n, i = read_varlen(data, i)
                i += n
                events.append(Event(tick, data[start:i]))
                continue
            size = 1 if status & 0xF0 in (0xC0, 0xD0) else 2
            body = data[i:i + size]
            i += size
            events.append(Event(tick, bytes([status]) + body))  # running status undone
        tracks.append(events)
        i = end
    return fmt, division, tracks


def write_midi(fmt: int, division: int, tracks: list[list[Event]]) -> bytes:
    out = bytearray(b"MThd" + struct.pack(">IHHH", 6, fmt, len(tracks), division))
    for events in tracks:
        body, last = bytearray(), 0
        for e in events:
            body += varlen(e.tick - last) + e.data
            last = e.tick
        out += b"MTrk" + struct.pack(">I", len(body)) + body
    return bytes(out)


def meta(tick: int, kind: int, payload: bytes) -> Event:
    return Event(tick, bytes([0xFF, kind]) + varlen(len(payload)) + payload)


def meta_text(e: Event) -> str:
    n, i = read_varlen(e.data, 2)
    return e.data[i:i + n].decode("utf-8", "replace")


def tempo_of(e: Event) -> int:
    return int.from_bytes(e.data[3:6], "big")


# --- the rewrite of §3.3 -------------------------------------------------------

def lead_in_ticks(division: int) -> int:
    """A bar less the pickup, when there is one."""
    return (BAR_QUARTERS - PICKUP_QUARTERS) * division if PICKUP_QUARTERS else 0


def part_of(index: int, events: list[Event]) -> tuple[str, int, int, int] | None:
    """The part of PARTS that a track is, by its name or its drum channel."""
    name = next((meta_text(e) for e in events if e.meta() == 0x03), None)
    channels = {e.channel() for e in events if e.channel() is not None}
    if index == 0:
        return None
    if channels == {9}:
        return PARTS["drums"]
    return PARTS.get(name or "")


def lyrics_only(events: list[Event]) -> bool:
    has_notes = any(e.kind() == 0x90 for e in events if e.channel() is not None)
    return not has_notes and any(e.meta() == 0x05 for e in events)


def tempo_ratio(tracks: list[list[Event]], studio: bool) -> float:
    """What every tempo in µs is multiplied by to play the score at BPM.

    The studio's is atTempo's (playbackSetup.ts, D155): the score's first
    tempo to the hundredth, over BPM. Without `studio`, the first tempo is
    taken as it is, as the run of §4.2 had it."""
    first_us = next(tempo_of(e) for e in tracks[0] if e.meta() == 0x51)
    if studio:
        return (round(6e9 / first_us) / 100) / BPM
    return (60_000_000 / BPM) / first_us


def rewrite(division: int, tracks: list[list[Event]], lean: bool = False,
            studio: bool = False) -> tuple[list[list[Event]], list[str]]:
    """lilypond's tracks as the studio would export them for GarageBand.

    `lean` also drops the words' track and the control track's name and text
    events, which GarageBand makes into empty piano tracks of their own (§4.3).
    `studio` is the rewrite Lily Studio makes (D162, `midi_export.rs`): lean,
    without CC 7 and CC 10 (§6.3), and the tempos scaled by atTempo's ratio."""
    log = []
    lean = lean or studio
    lead_in = lead_in_ticks(division)
    ratio = tempo_ratio(tracks, studio)
    out = []
    for index, events in enumerate(tracks):
        name = next((meta_text(e) for e in events if e.meta() == 0x03), None)
        channels = {e.channel() for e in events if e.channel() is not None}
        has_notes = any(e.kind() == 0x90 for e in events if e.channel() is not None)
        has_lyrics = any(e.meta() == 0x05 for e in events)
        part = part_of(index, events)

        if lean and has_lyrics and not has_notes:
            log.append("track %d %r dropped (lyrics only)" % (index, name))
            continue
        setup, rest = [], []
        for e in events:
            if lean and index == 0 and e.meta() in (0x01, 0x02, 0x03):
                continue
            if e.meta() == 0x03 and (part or (has_lyrics and not has_notes)):
                continue  # the name is written again below
            if e.kind() == 0xC0 and part:
                continue  # the program is written again below
            if e.meta() == 0x51:  # scale to % BPM:, as atTempo does (D155)
                us = round(tempo_of(e) * ratio)
                log.append("tempo at tick %d: %.2f -> %.2f bpm"
                           % (e.tick, 60e6 / tempo_of(e), 60e6 / us))
                e = Event(e.tick, b"\xFF\x51\x03" + us.to_bytes(3, "big"))
            # The lead-in: what sounds moves later, words and markers with it;
            # what sets up the track stays at 0.
            moves = e.meta() in (0x05, 0x06, 0x2F) or e.kind() in (0x80, 0x90, 0xA0, 0xD0, 0xE0)
            if e.tick == 0 and not moves:
                setup.append(e)
                continue
            rest.append(Event(e.tick + lead_in, e.data))

        head = []
        if part:
            label, program, volume, pan = part
            ch = min(channels)
            # A program on each channel the track plays, which is one for
            # lilypond's tracks unless the score maps its voices to channels.
            head = [meta(0, 0x03, label.encode())]
            head += [Event(0, bytes([0xC0 | c, program])) for c in sorted(channels)]
            if not studio:
                head += [Event(0, bytes([0xB0 | ch, 7, volume])),
                         Event(0, bytes([0xB0 | ch, 10, pan]))]
            log.append("track %d %r -> %r, channel %d, program %d, CC7 %d, CC10 %d"
                       % (index, name, label, ch, program, volume, pan))
        elif has_lyrics and not has_notes:
            head = [meta(0, 0x03, LYRICS_NAME.encode())]
            log.append("track %d %r -> %r (lyrics only)" % (index, name, LYRICS_NAME))
        elif index:
            log.append("track %d %r left as it was" % (index, name))
        out.append(head + setup + rest)
    log.append("lead-in: %d ticks (%d quarters)" % (lead_in, lead_in // division))
    return out, log


def tempo_map(division: int, control: list[Event]) -> list[tuple[int, float, float]]:
    """(tick, seconds, seconds a tick) for each tempo event."""
    points, seconds, last_tick, last_rate = [], 0.0, 0, None
    for e in control:
        if e.meta() != 0x51:
            continue
        if last_rate is not None:
            seconds += (e.tick - last_tick) * last_rate
        last_tick, last_rate = e.tick, tempo_of(e) / 1e6 / division
        points.append((e.tick, seconds, last_rate))
    return points


def seconds_at(points: list[tuple[int, float, float]], tick: float) -> float:
    """Ticks before the first tempo go at the first tempo: a count-in."""
    tick0, sec0, rate = points[0]
    for t, s, r in points:
        if t <= tick:
            tick0, sec0, rate = t, s, r
    return sec0 + (tick - tick0) * rate


# --- the synthetic recording and its stems ----------------------------------------

def recording(points, division: int, lead_in: int, end_tick: int, rate: int) -> dict[str, list[float]]:
    """The `% Audio:` file: its time is the music's time less the offset (D155).

    The music's time is the MIDI's time less the lead-in. A negative offset
    means the recording has that long before the music's 0, so the count-in
    clicks before it are synthesized too."""
    lead_s = seconds_at(points, lead_in)
    def rec_time(tick: float) -> float:
        return seconds_at(points, tick) - lead_s - AUDIO_OFFSET

    length = rec_time(end_tick) + 1.0
    n = int(length * rate)
    drums, bass = [0.0] * n, [0.0] * n
    beat = -(lead_in // division) - 8  # some count-in, cut off at the file's start
    while True:
        tick = lead_in + beat * division
        t = rec_time(tick)
        if tick >= end_tick:
            break
        if t >= 0:
            downbeat = (tick // division) % BAR_QUARTERS == 0
            click(drums, t, 2000.0 if downbeat else 1000.0, 0.6 if downbeat else 0.4, rate)
            if downbeat and tick >= lead_in:
                dur = rec_time(tick + division) - t
                tone(bass, t, 98.0, dur, 0.35, rate)
        beat += 1
    song = [d + b for d, b in zip(drums, bass)]
    return {"Song": song, "Drums": drums, "Bass": bass}


def click(buf: list[float], t: float, freq: float, amp: float, rate: int) -> None:
    start, n = int(round(t * rate)), int(0.03 * rate)
    for i in range(min(n, len(buf) - start)):
        buf[start + i] += amp * math.sin(2 * math.pi * freq * i / rate) * (1 - i / n) ** 2


def tone(buf: list[float], t: float, freq: float, dur: float, amp: float, rate: int) -> None:
    start, n = int(round(t * rate)), int(dur * 0.9 * rate)
    for i in range(min(n, len(buf) - start)):
        x = 2 * math.pi * freq * i / rate
        env = min(1.0, i / (0.005 * rate)) * math.exp(-2.5 * i / rate)
        buf[start + i] += amp * env * (math.sin(x) + 0.5 * math.sin(2 * x) + 0.25 * math.sin(3 * x))


def align(samples: list[float], shift: float, rate: int) -> list[float]:
    """Pad (shift > 0) or cut (shift < 0) so the file's 0 is the MIDI's tick 0."""
    n = int(round(shift * rate))
    return [0.0] * n + samples if n >= 0 else samples[-n:]


def write_wav(path: str, samples: list[float], rate: int) -> None:
    frames = bytearray()
    for s in samples:
        v = max(-32767, min(32767, int(round(s * 32767))))
        frames += struct.pack("<hh", v, v)
    with wave.open(path, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(bytes(frames))


# --- the same MIDI and stems through LogicProFormatWriter (§12) -----------------

def part_notes(division: int, tracks: list[list[Event]]) -> list[tuple[str, list[tuple[int, int, int, int]]]]:
    """Each track with notes: its name and [(tick, pitch, velocity, length)]
    at 960 a quarter, which is what LPFW's MIDI regions take."""
    parts = []
    for events in tracks:
        name = next((meta_text(e) for e in events if e.meta() == 0x03), "")
        on: dict[tuple[int, int], tuple[int, int]] = {}
        notes = []
        for e in events:
            if e.kind() not in (0x80, 0x90) or e.channel() is None:
                continue
            key = (e.channel(), e.data[1])
            if e.kind() == 0x90 and e.data[2]:
                on[key] = (e.tick, e.data[2])
            elif key in on:
                start, velocity = on.pop(key)
                notes.append((start * 960 // division, e.data[1], velocity,
                              max(1, (e.tick - start) * 960 // division)))
        if notes:
            parts.append((name, sorted(notes)))
    return parts


def lpfw(lpfw_dir: str, out: str, bundle: str, division: int, lean: list[list[Event]]) -> None:
    """Write OUT/seed.logicx, audio.logicx and av.logicx with LPFW's own calls.

    Its code is imported from LPFW_DIR, a clone of LogicProFormatWriter (MIT),
    and is not copied here. Every stem starts at tick 0, as the MIDI does."""
    import plistlib
    sys.path.insert(0, os.path.abspath(lpfw_dir))
    from logicx import projectdata as lp  # noqa: E402  (LPFW, MIT, Jon Kubis)
    os.makedirs(out, exist_ok=True)
    lean_mid = os.path.join(bundle, "probes", "Lean.mid")
    stems = ["Song", "Drums", "Bass"]
    wavs = [os.path.join(bundle, s + ".wav") for s in stems]
    parts = part_notes(division, lean)
    # The control track: tempo and meter, at 960 a quarter.
    tempos = [(e.tick * 960 // division, 60e6 / tempo_of(e)) for e in lean[0] if e.meta() == 0x51]
    meters = [(e.tick * 960 // division, e.data[3], 1 << e.data[4]) for e in lean[0] if e.meta() == 0x58]

    # seed: LPFW's mixed seed with nothing added, as Logic saved it.
    lp.synthesize_av_region_bundle(None, os.path.join(out, "seed.logicx"), seed=1)
    # audio: the beatmap pipeline, one audio track per stem, from the 64-strip seed.
    lp.export_beatmap(lean_mid, wavs, os.path.join(out, "audio.logicx"), head_sync=0, verbose=False)
    # av: an instrument track per part with its notes, and an audio track per stem.
    # The mixed seed has one of each already (Inst 1 at slot 0x58, Audio 1 at
    # 0x5c), so one fewer of each is added and those two are renamed. LPFW
    # writes names as Latin-1, so it is handed the UTF-8 bytes GarageBand uses.
    def utf8(name: str) -> str:
        return name.encode("utf-8").decode("latin-1")
    av = os.path.join(out, "av.logicx")
    lp.synthesize_av_region_bundle(
        None, av, instruments=len(parts) - 1, audio=len(wavs) - 1,
        midi_regions=[(i + 1, notes, 0, utf8(name)) for i, (name, notes) in enumerate(parts)],
        audio_regions=[(i + 1, wav, 0) for i, wav in enumerate(wavs)],
        inst_names=[utf8(name) for name, _ in parts[1:]], audio_names=stems[1:], seed=1)
    pd_path = os.path.join(av, "Alternatives", "000", "ProjectData")
    with open(pd_path, "rb") as f:
        pd = lp.ProjectData.parse(f.read())
    for r in pd.records:
        if r.tag == b"ivnE" and lp._u32(r.raw, 0x08) in (0x580000, 0x5C0000):
            r.raw = lp._set_ivne_name(r.raw, utf8(parts[0][0] if lp._u32(r.raw, 0x08) == 0x580000 else stems[0]))
    pd.set_tempo_map(tempos, ppq=960)
    pd.set_meter_map(meters, ppq=960)
    with open(pd_path, "wb") as f:
        f.write(pd.serialize())
    meta_path = os.path.join(av, "Alternatives", "000", "MetaData.plist")
    with open(meta_path, "rb") as f:
        md = plistlib.load(f)
    md["BeatsPerMinute"] = round(tempos[0][1], 2)
    with open(meta_path, "wb") as f:
        plistlib.dump(md, f, fmt=plistlib.FMT_BINARY)
    print("LPFW: %d instrument tracks (%s), %d audio tracks, tempos %s, meter %s"
          % (len(parts), ", ".join(n for n, _ in parts), len(wavs),
             ", ".join("%.2f at %d" % (b, t) for t, b in tempos),
             ", ".join("%d/%d at %d" % (n, d, t) for t, n, d in meters)))
    print(out)


MUSICXML ="""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN"
  "http://www.musicxml.org/dtds/partwise.dtd">
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>Probe Flute</part-name>
      <midi-instrument id="P1-I1"><midi-program>74</midi-program></midi-instrument>
    </score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes><divisions>1</divisions><key><fifths>1</fifths></key>
        <time><beats>3</beats><beat-type>4</beat-type></time></attributes>
      <sound tempo="100"/>
      <note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note>
      <note><pitch><step>A</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note>
      <note><pitch><step>B</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note>
    </measure>
  </part>
</score-partwise>
"""

README = """{title}: MIDI and stems for GarageBand

1. Open "{title}.mid" in GarageBand (double-click it, or File > Open).
   That makes a project with the tempo map, the meter and a track per part.
2. Select Song.wav, Drums.wav and Bass.wav in Finder and drag them together
   into the empty area below the tracks, at the very start of bar 1.

Every file starts at the same moment, so nothing else needs placing.
The score's bar 1 is GarageBand's bar 2: bar 1 holds a lead-in and the pickup.
"""


def plan_of(division: int, tracks: list[list[Event]]) -> dict:
    """The plan the studio's renderer sends with the MIDI (midi_export.rs's
    GarageBandPlan): the tracks to drop, each part's name and program, the
    tempo ratio and the lead-in in ticks."""
    parts = []
    for index, events in enumerate(tracks):
        part = part_of(index, events)
        if part:
            parts.append({"track": index, "name": part[0], "program": part[1]})
    return {
        "drop": [i for i, events in enumerate(tracks) if i and lyrics_only(events)],
        "parts": parts,
        "tempoRatio": tempo_ratio(tracks, studio=True),
        "leadIn": lead_in_ticks(division),
    }


def fixture(out: str) -> int:
    """`--fixture DIR`: the sample score, lilypond's MIDI, the studio's plan
    and the rewrite that plan should give, for the session crate's tests."""
    import json
    os.makedirs(out, exist_ok=True)
    with open(os.path.join(out, "garageband.ly"), "w") as f:
        f.write(SCORE.format(bpm=BPM, offset="%.2f" % AUDIO_OFFSET, title=TITLE))
    subprocess.run(["lilypond", "-s", "garageband.ly"], cwd=out, check=True)
    os.remove(os.path.join(out, "garageband.pdf"))
    with open(os.path.join(out, "garageband.midi"), "rb") as f:
        fmt, division, tracks = read_midi(f.read())
    with open(os.path.join(out, "garageband.plan.json"), "w") as f:
        json.dump(plan_of(division, tracks), f, indent=2, ensure_ascii=False)
        f.write("\n")
    rewritten, log = rewrite(division, tracks, studio=True)
    with open(os.path.join(out, "garageband.expected.mid"), "wb") as f:
        f.write(write_midi(fmt, division, rewritten))
    for line in log:
        print(line)
    return 0


def main() -> int:
    args = sys.argv[1:]
    if len(args) == 2 and args[0] == "--fixture":
        return fixture(os.path.abspath(args[1]))
    lpfw_dir = None
    if len(args) == 3 and args[1] == "--lpfw":
        lpfw_dir = args.pop(2)
        args.pop(1)
    if len(args) != 1:
        print(__doc__.split("\n\n")[1], file=sys.stderr)
        return 2
    root = os.path.abspath(args[0])
    work = os.path.join(root, "work")
    bundle = os.path.join(root, TITLE + " (GarageBand)")
    probes = os.path.join(bundle, "probes")
    for d in (work, probes):
        os.makedirs(d, exist_ok=True)

    with open(os.path.join(work, "score.ly"), "w") as f:
        f.write(SCORE.format(bpm=BPM, offset="%.2f" % AUDIO_OFFSET, title=TITLE))
    subprocess.run(["lilypond", "-s", "score.ly"], cwd=work, check=True)
    with open(os.path.join(work, "score.midi"), "rb") as f:
        original = f.read()
    shutil.copyfile(os.path.join(work, "score.midi"), os.path.join(probes, "lilypond.mid"))

    fmt, division, tracks = read_midi(original)
    rewritten, log = rewrite(division, tracks)
    with open(os.path.join(bundle, TITLE + ".mid"), "wb") as f:
        f.write(write_midi(fmt, division, rewritten))
    for line in log:
        print(line)
    lean, _ = rewrite(division, tracks, lean=True)
    with open(os.path.join(probes, "Lean.mid"), "wb") as f:
        f.write(write_midi(fmt, division, lean))

    lead_in = lead_in_ticks(division)
    points = tempo_map(division, rewritten[0])
    end_tick = max(e.tick for events in rewritten for e in events)
    lead_s = seconds_at(points, lead_in)
    shift = lead_s + AUDIO_OFFSET
    print("lead-in %.4f s; %s %.4f s of the recording"
          % (lead_s, "pad" if shift >= 0 else "cut", abs(shift)))
    print("score's bar 1 at MIDI %.4f s; bar 5 (the tempo change) at %.4f s"
          % (seconds_at(points, lead_in + PICKUP_QUARTERS * division),
             seconds_at(points, lead_in + (PICKUP_QUARTERS + 4 * BAR_QUARTERS) * division)))

    for rate, where in ((RATE, bundle), (48000, probes)):
        audio = recording(points, division, lead_in, end_tick, rate)
        if rate == RATE:
            write_wav(os.path.join(work, "Song (recorded).wav"), audio["Song"], rate)
            for stem, samples in audio.items():
                write_wav(os.path.join(where, stem + ".wav"), align(samples, shift, rate), rate)
        else:
            write_wav(os.path.join(where, "Drums 48k.wav"), align(audio["Drums"], shift, rate), rate)

    flac = os.path.join(probes, "Drums.flac")
    if shutil.which("afconvert"):
        subprocess.run(["afconvert", "-f", "flac", "-d", "flac",
                        os.path.join(bundle, "Drums.wav"), flac], check=True)
    with open(os.path.join(probes, "Probe.musicxml"), "w") as f:
        f.write(MUSICXML)
    with open(os.path.join(bundle, "READ ME.txt"), "w") as f:
        f.write(README.format(title=TITLE))
    print(bundle)
    if lpfw_dir:
        lpfw(lpfw_dir, os.path.join(root, "lpfw"), bundle, division, lean)
    return 0


if __name__ == "__main__":
    sys.exit(main())
