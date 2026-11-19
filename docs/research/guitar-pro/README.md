# Guitar Pro files in Lily Studio

What Lily Studio needs to know to import a Guitar Pro file as a score: the
binary layout of GP3, GP4 and GP5 (§2), the `.gpx` container of Guitar Pro 6
and the `score.gpif` inside it (§3), how both map onto the song model of
`packages/studio/crates/guitarpro` (§4), and what the samples hold that a
reader or writer would not expect (§5). D166 in `docs/DECISIONS.md` sets out
the import and its stages.

**[sample]** marks what was checked against the samples of §1 with
`spike/inspect_gp.py` on 2026-11-04: its walk of a GP3/4/5 file reads every
section of §2 in order and fails on the first byte it cannot place, so a walk
that ends at the file's end (§1) checks the layout of every branch it took.
**[described]** marks what comes only from public prose descriptions of the
formats and was not met in a sample; a reader should treat it as likely, and
fail clearly rather than guess when it does not fit. **[inferred]** marks a
meaning worked out from the samples' values rather than read anywhere.

The studio is MIT. This note and the spike were written for this repository;
no code of TuxGuitar, PyGuitarPro, alphaTab or any other LGPL, MPL or GPL
project was copied, and the readers must be written from this note and the
bytes, not from such code (D166).

## 1. The samples

Copyrighted songs, kept out of the repository (D166). The crate's tests read
them from `GUITAR_PRO_SAMPLES` (unset, none is found) and skip
those not found.

| File | Header | Tracks | Bars | Walk |
| --- | --- | --- | --- | --- |
| `gp3-a.gp3` | `FICHIER GUITAR PRO v3.00` | 1 | 25 | whole, 4 zero bytes after (§2.11) |
| `gp3-b.gp3` | `FICHIER GUITAR PRO v3.00` | 4 | 142 | whole, 4 zero bytes after |
| `gp4-a.gp4` | `FICHIER GUITAR PRO v4.00` | 6 | 100 | whole, 4 zero bytes after |
| `gp5-a.gp5` | `FICHIER GUITAR PRO v5.00` | 10 | 139 | whole, to the last byte |
| `gp5-b.gp5` | `FICHIER GUITAR PRO v5.10` | 4 | 80 | whole, to the last byte |
| `gp5-c.gp5` | `FICHIER GUITAR PRO v5.10` | 7 | 127 | whole, to the last byte |
| `gpx-a.gpx` | `BCFZ` | 4 | 17 | BCFZ → BCFS → `score.gpif`, GPIF revision 9934 |

What the walks met, so what §2 checks [sample]: rests and empty beats,
dotted beats, triplets, beat text, mix-table changes (with a tempo change in
GP3 and GP4, none in GP5), strokes, beat vibrato, ties, dead and ghost notes,
note dynamics, bends, grace notes, hammers,
slides (GP3 flag, GP5 kinds 1, 2 and 4), let ring, palm mute, artificial
harmonics (GP5), note vibrato, GP5's second voice, markers, a repeat (GP3),
keys in bar headers, drum tracks, 4-string basses and 6-string drop tunings. Not
met, so [described] below: chord diagrams in GP3–GP5, alternate endings,
GP5 repeats, a GP5 tempo change, trills, tremolo picking, tapped and pinch
harmonics, slaps, pick strokes, the tremolo bar, 7-string tracks, a capo.

    python3 -I docs/research/guitar-pro/spike/inspect_gp.py FILE.gp5 [--beats] [--encoding cp1251]
    python3 -I docs/research/guitar-pro/spike/inspect_gp.py FILE.gpx [--xml OUT.gpif]

## 2. GP3, GP4 and GP5

### 2.1 Values and strings

Everything is little-endian and unaligned [sample]. The readers need:

| Name here | Bytes |
| --- | --- |
| `byte` | 1, unsigned |
| `sbyte` | 1, signed |
| `bool` | 1, 0 or not |
| `short` | 2, signed |
| `int` | 4, signed |
| `double` | 8, IEEE 754 |
| `ByteString(n)` | a length byte, then a field of exactly `n` bytes, of which the first *length* are the text |
| `IntString` | an `int` length, then that many bytes |
| `IntByteString` | an `int` *n* (the length + 1), a length byte, then *n* − 1 bytes, of which the first *length* are the text |
| `Color` | 4 bytes: red, green, blue, 0 |

**Text has no declared encoding** [sample]. It is the ANSI code page of the
machine that saved it. `gp5-b.gp5` is Windows-1251 (its title and its
markers *Интро*, *Куплет*,
*Припев*), the others plain ASCII or Windows-1252. A reader should take
UTF-8 when the bytes are valid UTF-8 with some byte ≥ 0x80, else guess
between 1251 and 1252: in Russian text most letters are ≥ 0xC0, in western
text few are. The model holds `String`s, so the guess is the reader's.

### 2.2 Header

`ByteString(30)`: `FICHIER GUITAR PRO v3.00`, `v4.00`, `v4.06`, `v5.00`,
`v5.10` [sample for 3.00, 4.00, 5.00, 5.10]. The first byte is the length (24
= `0x18`), and the field is padded to 31 bytes with whatever was in memory
(zeros, or junk such as `3A 88 6F 70 2D` in `gp3-b.gp3`). `crate::sniff`
reads it. GP1 and GP2 begin `FICHIER GUITARE PRO` or `v1.x`/`v2.x` and are
refused, as is GP7's `.gp` (a zip, `PK\3\4`).

### 2.3 Song information

`IntByteString`s, in this order [sample]:

- GP3, GP4: title, subtitle, artist, album, words (*words and music*),
  copyright, tab (the transcriber), instructions.
- GP5: title, subtitle, artist, album, words, music, copyright, tab,
  instructions.

Then the notice: an `int` count and that many `IntByteString` lines.

### 2.4 Triplet feel (GP3, GP4)

A `bool`: the whole song swings eighths [sample]. GP5 has it per master bar
instead (§2.8).

### 2.5 Lyrics (GP4, GP5)

An `int`, the track the words belong to (from 1; 0 in `gp4-a.gp4`,
whose one line is not attached [inferred]), then 5 lines, each an `int`
(the bar it starts at, from 1) and an `IntString` (the text, lines split by
`\r\n`) [sample].

### 2.6 RSE master effect (v5.10) and page setup (GP5)

v5.10 only: an `int` master volume (100), an `int` (0), and 11 `sbyte`s of
equalizer (10 bands and the gain) [sample, 5.10; absent in 5.00].

GP5: page setup [sample]: 7 `int`s (page width and height in mm, margins
left, right, top, bottom, the score's size in percent; `210 297 10 10 15 10
100`), a `short` of which header and footer lines are shown (`0x1FF`), then
10 `IntByteString`s, the templates for title, subtitle, artist, album,
words, music, words-and-music, copyright (two lines) and page number
(`%TITLE%`, `Page %N%/%P%`, …). Nothing of it is imported.

### 2.7 Tempo and key

- GP5: the tempo's name, an `IntByteString` [sample].
- The tempo, an `int`, quarter notes a minute [sample: 95, 127, 120].
- v5.10: a `bool`, the tempo hidden [sample].
- GP3: the key, an `int` [sample].
- GP4, GP5: the key, an `sbyte` (fifths), then 4 bytes: three blanks and an
  octave byte, all zero in every sample [sample].

### 2.8 MIDI channels, directions, counts

64 channels, 4 ports of 16, each 12 bytes [sample]: `int` program (General
MIDI, from 0), then `byte`s volume, balance, chorus, reverb, phaser,
tremolo, and 2 blank bytes. Volume and balance are on Guitar Pro's 0–16
scale (13 and 8 are the defaults; 8 is the middle); MIDI's 0–127 is about
`value × 8`, less 1 above 0 [inferred]. Drums sit on the tenth channel.

GP5 then has the directions, 19 `short`s, each the master bar (from 1) a
mark is at or −1 [sample: all −1]. In order [described]: Coda, Double Coda,
Segno, Segno Segno, Fine, Da Capo, Da Capo al Coda, Da Capo al Double Coda,
Da Capo al Fine, Da Segno, Da Segno al Coda, Da Segno al Double Coda, Da
Segno al Fine, Da Segno Segno, Da Segno Segno al Coda, Da Segno Segno al
Double Coda, Da Segno Segno al Fine, Da Coda, Da Double Coda. Then an `int`,
the master reverb [sample].

All formats: an `int` count of master bars, then an `int` count of tracks
[sample].

### 2.9 Master bars

One header per bar, in order [sample]. GP5 puts one blank byte before every
header but the first. Then a flags `byte`:

| Flag | Meaning | Data, GP3/GP4 order |
| --- | --- | --- |
| `0x01` | numerator | `sbyte` |
| `0x02` | denominator | `sbyte` |
| `0x04` | repeat starts | — |
| `0x08` | repeat ends | `sbyte` count |
| `0x10` | alternate ending | `byte` |
| `0x20` | marker | `IntByteString` name, `Color` |
| `0x40` | key change | `sbyte` fifths, `sbyte` 0 major / 1 minor |
| `0x80` | double bar | — |

A meter or key not given is the previous bar's. GP3 and GP4 read the data in
the order of the table [sample]. **GP5 reads it in another order**
[sample]: numerator, denominator, repeat end, marker, key, alternate
ending; then, when `0x01` or `0x02` is set, 4 bytes of beaming (how many
eighths each beam group holds, `2 2 2 2` for 4/4); then one blank byte when
`0x10` is *not* set (the alternate ending's byte takes its place); then the
triplet feel, a `byte` (0 none, 1 eighths, 2 sixteenths).

**Repeat counts differ** [sample for GP3, described for GP5]. In
`gp3-a.gp3` the repeat of bars 1–7 ends with 1, and
`gpx-a.gpx` writes the same repeat as `count="2"`: GP3
and GP4 store the repeats (plays − 1), GPX the plays. GP5 is described as
storing the plays. The model keeps the plays.

**Alternate endings** [described]: in GP3 and GP4 the `byte` is the ending's
number; in GP5 a bitmask, bit 0 the first ending.

Markers in GP3/GP4 are black (`0 0 0 0`), in GP5 red (`255 0 0 0`)
[sample].

### 2.10 Tracks

One per track, in order [sample]. GP5 puts a blank byte before the first
track (v5.10) or before every track (v5.00).

- flags `byte`: `0x01` drums, `0x02` 12-string, `0x04` banjo [described];
  GP5 adds `0x08` (set on every track), `0x40` (RSE), `0x80` (seen with
  RSE in v5.00) [sample values: `0x00`, `0x01` in GP3/4; `0x08`, `0x09`,
  `0x48`, `0x49`, `0x88`, `0xC8` in GP5]
- name, `ByteString(40)`
- string count, `int`, then 7 `int` tunings as MIDI pitches, the first
  string (highest) first; only the first *count* are used [sample: `64 59 55
  50 45 40`, bass `43 38 33 28`, `gp5-a.gp5`'s `61 56 52 47 42 35` and bass `40
  35 30 23`]. Drum tracks have six zeros.
- port, `int` from 1; channel, `int` from 1 (into §2.8's table:
  `(port − 1) × 16 + channel − 1`); effect channel, `int` from 1 [sample]
- fret count, `int` (24; 87 or 99 on drums) [sample]
- capo, `int` [sample: always 0]. Frets are written above the capo, so a
  note sounds at tuning + capo + fret [described].
- `Color`

GP5 then has [sample, both versions]:

- a `short` of display flags, a `byte` auto-accentuation, a `byte` MIDI
  bank;
- RSE: a `byte` humanize, 3 `int`s, 12 unknown bytes, then the RSE
  instrument: `int` instrument, `int`, `int` sound bank;
- v5.00: a `short` effect number and one blank byte;
- v5.10: an `int` effect number, 4 equalizer bytes, then two
  `IntByteString`s, the effect and its category (*American Clean -
  Default*, *Amp Tones*).

After the last track GP5 has 2 blank bytes (v5.00) or 1 (v5.10) [sample].

### 2.11 Bars of the tracks

For each master bar, for each track, that track's bar [sample]:

- GP3, GP4: one voice: an `int` count of beats, then the beats.
- GP5: two voices, each an `int` count and its beats, then one `byte`
  (described as a line break; always 0 in the samples). **The file's last
  bar has no such byte**: every GP5 sample ends right after its last voice
  [sample].
- **GP3 and GP4 end with 4 zero bytes** after the last bar [sample, all
  three files]. A reader should accept them.

### 2.12 Beats

A flags `byte`, then [sample]:

| Flag | Data, in order |
| --- | --- |
| `0x40` | status `byte`: 0 empty, 1 normal, 2 rest |
| (always) | duration `sbyte`: −2 whole, −1 half, 0 quarter, 1 eighth, 2 sixteenth, 3 32nd, 4 64th |
| `0x20` | tuplet `int`: 3, 5, 6, 7, 9, 10, 11, 12, 13 notes |
| `0x02` | chord (§2.16) |
| `0x04` | text, `IntByteString` |
| `0x08` | beat effects (§2.15) |
| `0x10` | mix-table change (§2.17) |
| (always) | strings `byte`: bit 6 the first string, bit 0 the seventh |
| | a note (§2.13) for each bit set, from bit 6 down |
| GP5 (always) | a `short` of display flags; when `0x0800` is set, one `byte` more [described] |

`0x01` is a dot. The tuplet is *n* in the time of 2 (3), 4 (5, 6, 7) or 8
(9–13) [described; the samples have only 3]. An empty beat (status 0) is
what fills GP5's second voice where nobody wrote one: 426 of them in the
`gp5-b.gp5`, 2051 in `gp5-a.gp5`'s; it takes its duration but shows nothing
[sample].

### 2.13 Notes

A flags `byte`, then [sample, but for `0x80` and GP5's `0x01`]:

| Flag | GP3, GP4 | GP5 |
| --- | --- | --- |
| `0x20` | type `byte`: 1 normal, 2 tie, 3 dead | the same |
| `0x01` | duration `sbyte` and tuplet `sbyte` of its own | (later) |
| `0x10` | dynamic `sbyte`, 1 ppp … 8 fff | the same |
| `0x20` | fret `sbyte` | the same |
| `0x80` | left and right fingering, 2 `sbyte`s | the same |
| `0x01` | — | a `double`, the note's length in percent |
| (always) | — | one flags `byte` (`0x02` swaps the accidental) |
| `0x08` | note effects (§2.14) | the same |

Other flags: `0x04` ghost, `0x40` accent; in GP5 `0x02` a heavy accent.
GP3 and GP4 also set `0x02`, on normal notes only, on every kind of track
(571 notes in `gp3-b.gp3`, 271 in `gp4-a.gp4`'s); what it means there was not
found, and a reader should ignore it [sample]. A note without `0x10` plays
at forte, dynamic 6 [described]. A tied note's fret repeats the note it
holds (or is 0); take the previous note on the string [described].

### 2.14 Note effects

GP3: one flags `byte`. GP4, GP5: two [sample]. Then, in order:

| Flag | Meaning | Data |
| --- | --- | --- |
| 1: `0x01` | bend | §2.15's curve |
| 1: `0x02` | hammer-on or pull-off to the next note | — |
| 1: `0x04` | GP3 only: slide to the next note | — |
| 1: `0x08` | let ring | — |
| 1: `0x10` | grace note | `byte` fret, `byte` dynamic, `byte` transition (0 none, 1 slide, 2 bend, 3 hammer), `byte` duration (1 a 64th, 2 a 32nd, 3 a 16th [described]); GP5 adds a flags `byte` (`0x01` dead, `0x02` on the beat) |
| 2: `0x01` | staccato | — |
| 2: `0x02` | palm mute | — |
| 2: `0x04` | tremolo picking | `byte`: 1 eighths, 2 sixteenths, 3 32nds [described] |
| 2: `0x08` | slide | `byte`; GP4 [described]: 1 shift, 2 legato, 3 out downwards, 4 out upwards, −1 into from below, −2 into from above; GP5 a bitmask: `0x01` shift, `0x02` legato, `0x04` out downwards, `0x08` out upwards, `0x10` into from below, `0x20` into from above [sample: 1, 2, 4] |
| 2: `0x10` | harmonic | `byte` kind: 1 natural, 2 artificial, 3 tapped, 4 pinch, 5 semi; GP5 adds 3 `byte`s to an artificial one (the note, its accidental, its octave) [sample] and one to a tapped one (the fret) [described]; GP4 [described] writes artificial ones as 15, 17, 22 (the interval) |
| 2: `0x20` | trill | `byte` fret, `byte` period (1 sixteenths, 2 32nds, 3 64ths) [described] |
| 2: `0x40` | vibrato | — |

**A bend** (and the tremolo bar's curve) is [sample]: a `byte` kind (1
bend, 2 bend and release, …), an `int` value, an `int` count, then each
point: an `int` position (0 to 60 across the note), an `int` value, a
`bool` vibrato. Values are 25 to the semitone, so 50 is a full bend [sample:
`gp3-b.gp3`'s and `gp5-b.gp5`'s are Guitar Pro's stock *Bend* (0, 50 at 30, 50 at
60) and *Bend and release* (0, 50 at 10, 50 at 20, 0 at 30, 0 at 60);
`gp5-c.gp5` bends 100, two tones; the unit is described and fits].

### 2.15 Beat effects

GP3: one flags `byte`. GP4, GP5: two [sample]. Then:

| Flag | Meaning | Data |
| --- | --- | --- |
| 1: `0x01` | vibrato | — (set with every note's vibrato in the GP5 samples) |
| 1: `0x02` | wide vibrato | — |
| 1: `0x04`, `0x08` | GP3 only: natural, artificial harmonic | — |
| 1: `0x10` | fade in | — |
| 1: `0x20` | GP3: tremolo bar or slap; GP4, GP5: slap | GP3: `byte` kind (0 tremolo bar, 1 tap, 2 slap, 3 pop) and an `int` value; GP4, GP5: `byte` kind (1 tap, 2 slap, 3 pop) [described] |
| 2: `0x04` | GP4, GP5: tremolo bar | a bend curve (§2.14) |
| 1: `0x40` | stroke | 2 `byte`s, one of them 0, each 1–6 for a 128th to a quarter [described]. GP3, GP4: the down strum's speed, then the up strum's [described]. GP5: the up strum's, then the down strum's [inferred: `gp5-a.gp5`'s 1147 strokes set the second byte 749 times and the first 398, and all 273 of `gp5-b.gp5`'s strummed chords set the second] |
| 2: `0x02` | GP4, GP5: pick stroke | `byte`: 1 up, 2 down [described] |
| 2: `0x01` | GP4, GP5: rasgueado | — |

### 2.16 Chords

None in the samples; all [described]. A `byte` comes first. 0 is the old
form: an `IntByteString` name, an `int` first fret, and when it is not 0, 6
`int` frets. 1 is the new form, always used by GP5:

- GP4, GP5 (107 bytes with the leading one): a `bool` sharp, 3 blank bytes,
  `byte`s root, type and extension, `int`s bass and tonality, a `bool` add,
  the name as `ByteString(21)`, `byte`s fifth, ninth, eleventh and a blank,
  an `int` first fret, 7 `int` frets (−1 for a string not played, the first
  string first), then 32 bytes of barres (a count and 5 frets, starts and
  ends), 7 omissions, a blank, 7 fingers and a `bool` showing them.
- GP3: the same fields with `int`s for root, type and extension, 6 frets
  and 2 barres; its exact size was not checked, and a reader should not
  guess it from GP4's.

Only the name, the first fret and the frets matter to the model.

### 2.17 Mix-table changes

[sample, but for GP5's tempo] An `sbyte` instrument (−1 for no change);
GP5 then has 16 bytes of RSE instrument. Then 6 `sbyte`s: volume, balance,
chorus, reverb, phaser, tremolo (−1 for no change). GP5: the tempo's name,
an `IntByteString`. Then the tempo, an `int` (−1 for no change). Then a
`byte` duration (in beats) for each of the six that changed, and one for the
tempo if it changed; v5.10 adds a `bool` after the tempo's (the tempo
hidden) [described]. GP4 and GP5 then have a `byte` of flags, which changes
apply to all tracks. GP5 then a `byte` wah, and v5.10 two `IntByteString`s
(the RSE effect and its category).

A tempo change happens at its beat [sample]: `gp3-a.gp3`
changes tempo on the first beat of bar 23 of 25, `gp4-a.gp4` on bar
99 of 100. `gp4-a.gp4`'s other mix tables change only instruments or volumes.

### 2.18 v5.00 against v5.10

| | v5.00 | v5.10 |
| --- | --- | --- |
| RSE master effect after the lyrics | none | 19 bytes (§2.6) |
| hidden tempo after the tempo | none | a `bool` |
| blank before a track | every track | the first only |
| a track's RSE end | `short` effect, 1 blank | `int` effect, 4 EQ bytes, 2 strings |
| blank after the tracks | 2 bytes | 1 byte |
| mix table end | wah | wah, 2 strings; a `bool` after the tempo's duration |

All but the mix table's tempo [sample: `gp5-a.gp5`, v5.00, against `gp5-b.gp5`'s
and `gp5-c.gp5`, v5.10].

## 3. GPX (Guitar Pro 6)

### 3.1 BCFZ

A `.gpx` is `BCFZ`, then an `int` (little-endian), the unpacked size
(135 172 for a 37 089-byte file), then a bit stream [sample]. Bits are read
from each byte's highest bit down. The stream holds two kinds of chunk until
the output reaches the size:

- bit 0: **literal**. 2 bits, low bit first, are a count *n* (0–3); then *n*
  bytes, 8 bits each, high bit first.
- bit 1: **copy**. 4 bits, high bit first, are a width *w*; then *w* bits,
  low bit first, an offset back from the end of the output, and *w* bits, low
  bit first, a length. Copy `min(offset, length)` bytes from `output.len() −
  offset`, one at a time.

The output is cut to the size. It begins `BCFS`. A file that begins `BCFS`
itself is the same image uncompressed [described].

### 3.2 BCFS

After its 4-byte magic, the image is sectors of `0x1000` bytes, counted
from 0 [sample]. A sector that holds an entry has:

| Offset | Value |
| --- | --- |
| `0x00` | `int` kind: 1 directory, 2 file |
| `0x04` | the name, NUL-terminated, up to 127 bytes |
| `0x88` | `int` 1 [inferred, unknown] |
| `0x8C` | `int` size in bytes |
| `0x90` | `int` 0 |
| `0x94` | `int` sector numbers of its data, up to a 0 |

The root directory `/` is sector 1. Its data is a list of `int`s, the
sectors of its entries: `2, 23, 25, 29, 31` in the sample [sample]. Its files
are `score.gpif` (77 727 bytes of XML), `misc.xml`, `BinaryStylesheet`,
`PartConfiguration` and `LayoutConfiguration`. Only `score.gpif` is needed.

**Walk the directory; do not scan the sectors** [sample]. The common
description finds files by looking for kind 2 at every sector. The
directory's own data sector begins with `2` (its first entry's number), so
the scan finds a phantom file at sector 3 with a junk name and size 0.

### 3.3 `score.gpif`

UTF-8 XML, text in CDATA [sample]. The root `GPIF` holds, in order:
`GPRevision` (9934), `Score`, `MasterTrack`, `Tracks`, `MasterBars`, `Bars`,
`Voices`, `Beats`, `Notes`, `Rhythms`. The last six are flat lists whose
items have an `id` and refer to one another by it:

    MasterBar ─Bars─▶ Bar (one per track, in track order)
    Bar ─Voices─▶ Voice (4 ids, -1 for none)
    Voice ─Beats─▶ Beat
    Beat ─Rhythm ref─▶ Rhythm,  ─Notes─▶ Note
    Beat ─Chord─▶ the track's DiagramCollection item

**Items are shared** [sample]: beats 1 and 2 both list note 1, and the 117
beats use 6 rhythms. A reader resolves ids and copies; it must not assume an
id is used once. **They are shared across tracks too**: note 3 is the
bass's first F (string 0 of four) and, in beat 6, the low F of the rhythm
guitar's barre (string 0 of six); beat 1 is in the lead guitar's voice 0 and
the bass's voice 5. So a note's string is turned to the model's by the
string count of the track it is read for, not once for the file. The reader
copies within a budget of beats and notes, so that a few kilobytes sharing
one voice everywhere cannot grow without bound.

### 3.4 Score

`Title`, `SubTitle`, `Artist`, `Album`, `Words`, `Music`, `WordsAndMusic`,
`Copyright`, `Tabber`, `Instructions`, `Notices`; then page headers and
footers as HTML, which are not imported, `ScoreSystemsDefaultLayout`,
`ScoreSystemsLayout` and `PageSetup` [sample].

### 3.5 MasterTrack

`Tracks` (`0 1 2 3`), `Automations` and `RSE` [sample]. A tempo is an
`Automation` with `Type` `Tempo`, `Bar` (from 0), `Position`, `Linear`,
`Visible` and `Value` `95 2`: the tempo and its beat's unit, 2 being the
quarter [sample for the value; the unit described: 1 eighth, 2 quarter, 3
dotted quarter, 4 half, 5 dotted half]. `Position` is 0 in the sample; it
is described as a fraction of the bar [described].

### 3.6 Tracks

Each `Track id` holds [sample]: `Name`, `ShortName`, `Color`,
`Instrument ref` (`e-gtr6`, `s-gtr6`, `e-bass4`, `drmkt`), `PartSounding`
(`TranspositionPitch` −12 on guitars, the octave they are written above),
`RSE`, `GeneralMidi table="Instrument"` or `"Percussion"` (`Program`, `Port`,
`PrimaryChannel` from 0, 9 for drums, `SecondaryChannel`), `Lyrics` (5
`Line`s of `Text` and `Offset`, the bar), and `Properties`:

- `Tuning`: `Pitches`, **lowest string first** (`40 45 50 55 59 64`), the
  reverse of GP3–GP5. Absent on drums.
- `CapoFret`: `Fret` [described; absent in the sample].
- `DiagramCollection`: `Items` of `Item id name` with a `Diagram`
  (`stringCount`, `baseFret`, `Fret string fret` with string 0 the lowest)
  and a `Chord`. A beat's `Chord` is an item's id [sample]. The sample's
  *F* has `baseFret="0"` and frets 1 3 3 2 1 1, so frets there are the
  neck's; whether they count from `baseFret` when it is not 0 was not seen,
  and the reader adds it.

`Name` and `ShortName` may end in a space (a track of `gpx-a.gpx`) [sample]; the
reader trims them. Volume and pan are not in `GeneralMidi` but in the RSE
`ChannelStrip`'s 16 `Parameters`: the twelfth is the pan and the thirteenth
the volume, 0 to 1 (`0.5`, `0.795`), as the strip's `DSPParam_11` and
`DSPParam_12` automations repeat them [inferred].

### 3.7 MasterBars

Each `MasterBar` [sample unless marked]: `Key` (`AccidentalCount` −1,
`Mode` `Major`), `Time` (`4/4`), `Bars`, and when they apply `Repeat start
end count` (the count is the plays; a repeat may end with no start since the
last end, as bars 8–15 of the sample do, which repeat from bar 8 [sample]),
`Directions` with `Target`s (`Segno`,
`Coda`) and `Jump`s [described: `DaCapo`, `DalSegnoAlCoda`, `DaCoda`, …],
`AlternateEndings` (`1 2`) [described], `Section` with `Letter` and `Text`
[described], `TripletFeel` (`Triplet8th`, `Triplet16th`, …) [described],
`DoubleBar` [described], `Fermatas` [described].

### 3.8 Bars, Voices, Beats, Notes, Rhythms

- `Bar`: `Clef` (`G2`, `F4`, `Neutral` on drums), `Voices` [sample].
- `Voice`: `Beats` [sample].
- `Beat`: `Rhythm ref`, `Notes` (absent on a rest), `Dynamic` (`MF`), `Chord`,
  `Bank`, `Properties` [sample]; `GraceNotes` (`BeforeBeat`, `OnBeat`),
  `FreeText`, `Tremolo` (`1/8`), `Fadding`, `Arpeggio`, `Wah`, and
  properties `Brush` (`Direction`), `PickStroke`, `Rasgueado`, `Slapped`,
  `Popped`, `WhammyBar…` [described].
- `Note`: `Properties` with `String` (**0 the lowest**) and `Fret`, or on
  drums `Element` and `Variation`; `Tie origin destination`, `Accidental`
  [sample]; `Vibrato` (`Slight`, `Wide`), `LetRing`, `AntiAccent`
  (`Normal`, a ghost), `Accent` (a bitmask: 1 staccato, 4 heavy, 8 accent),
  `Trill`, and properties `Muted`, `PalmMuted`, `Slide` (`Flags`, §2.14's
  GP5 bitmask), `HopoOrigin`, `HopoDestination`, `Bended` with
  `BendOriginValue`, `BendMiddleValue`, `BendDestinationValue` and their
  offsets (values 50 to the semitone? unchecked), `HarmonicType`,
  `HarmonicFret`, `Tapped` [described].
- `Rhythm`: `NoteValue` (`Whole`, `Half`, `Quarter`, `Eighth`, `16th`,
  `32nd`, `64th`), `AugmentationDot count`, `PrimaryTuplet num den`
  [sample but the tuplet, described].

**String 0 is the lowest** [sample]: beat 6 of the acoustic guitar is
strings 0–5 at frets 1 3 3 2 1 1. Counted from the lowest string that is
F C F A C F, the barre F of the song's key and its diagram *F* in the same
track; counted from the highest it is no chord. All of that track's notes
fit F major counted from the lowest, and fit it worse the other way.

**Drums** [sample]: notes are `Element` and `Variation` (`0`–`15`, `0`), not
MIDI keys. Element is an index into Guitar Pro 6's drum kit, which the file
does not carry. The GPX reader needs a table from (element, variation) to
General MIDI's key, written by us from Guitar Pro 6's drum-kit names (kick,
snare, hi-hat closed, …) and checked against a GP6 export of a known
pattern; until then, drums from a `.gpx` are a known gap. The sample cannot
check such a table: `gpx-a.gpx` has no drums, and its drum track
plays the tune on the kit's elements as on pitches (5 0 0 for its first three notes,
G F F). The reader leaves drum notes out, and their beats become rests that
keep the time.

## 4. Into the model

`crates/guitarpro/src/model.rs` is the contract. The conversions the
readers make:

| Model | GP3–GP5 | GPX |
| --- | --- | --- |
| `Note::string`, 1 the highest | as stored | `string_count − String` |
| `Track::tuning`, highest first | as stored, cut to the count | `Pitches` reversed |
| `Duration::value` | `4 >> −d` for `d < 0`, else `4 << d` | `NoteValue` |
| `Tuplet { notes, space }` | §2.12's table | `PrimaryTuplet num den` |
| `MasterBar::repeat_end`, the plays | GP3/GP4 count + 1; GP5 as stored | `count` |
| `MasterBar::alternate_endings` | GP3/GP4 the number; GP5 the bits | `AlternateEndings` |
| `KeySignature` | the bar's, else the last; minor from the second byte | `AccidentalCount`, `Mode` |
| `Song::tempo`, `tempo_changes` | the song's `int`; mix tables' tempos at their beat's offset | automations at `Bar` and `Position` |
| `Note::dynamic` | `Dynamic::from_step`, forte when absent | the beat's `Dynamic` |
| `BendPoint` | position / 60, value / 25 semitones | from the bend values (§3.8) |
| `Note::fret` on drums | the fret (it is the GM key) | §3.8's table |
| `Track::midi` | §2.8's channel, port and channel from 0 | `GeneralMidi` |
| `Track::lyrics` | the GP4/GP5 lines on their track, bars from 0 | `Lyrics`' lines |
| grace notes | a note's `Grace` | a `GraceNotes` beat becomes the next beat's notes' `Grace` on the same strings |
| text | §2.1's guess | UTF-8 |

`Song::version` keeps the header (`FICHIER GUITAR PRO v5.10`) or
`GPIF 9934`.

## 5. What the samples show

### 5.1 Not every track is a guitar

GP3–GP5 store every track as frets on strings, so a piano, voice or string
section has a guitar's tuning and a guitar's six strings [sample]:

| File | Track | Program | Tuning |
| --- | --- | --- | --- |
| `gp4-a.gp4` | Piano | 1 (bright piano) | standard guitar |
| `gp4-a.gp4` | Strings | 41 (viola) | standard guitar |
| `gp5-c.gp5` | Vox | 70 (bassoon) | standard guitar |
| `gp5-c.gp5` | Backing Vox | 66 (tenor sax) | standard guitar |
| `gp5-c.gp5` | Keyboard | 20 (reed organ) | standard guitar |
| `gp5-a.gp5` | FX ×2 | 84, 80 (synth leads) | the band's drop tuning |

Neither the flags nor the tuning tell them apart from a guitar; the MIDI
program does. D166 takes a track as `TrackKind::Strings` when its program
is a guitar or bass (24–39) or another fretted instrument (104–107: sitar,
banjo, shamisen, koto), or its GPX instrument is one (`…-gtr…`, `…-bass…`,
banjo, ukulele); `Drums` when the drum flag, channel 10 or GPX's
`Percussion` table says so; and `Other` otherwise, which gets a Staff alone.

### 5.2 Other things a reader meets

- Drop tunings are common: `gp5-a.gp5`'s guitars are C♯ G♯ E B F♯ B and C♯ G♯ E
  B F♯ C♯, its bass E B F♯ B [sample].
- Two tracks may share a name (`gp5-a.gp5`'s two *FX*, two *Acoustic 01*)
  [sample]; the writer must make its variable names unique.
- Most of a GP5 file's second voices are empty beats (§2.12), so a writer
  should drop a voice that is all empty rather than fill it with spacers.
- `gp5-b.gp5`'s text is Windows-1251 (§2.1).
- The two files of one song (`gp3-a.gp3`, `gpx-a.gpx`) are different arrangements (1 track of 25
  bars against 4 tracks of 17), but both repeat bars 1–7, which fixed the
  repeat counts of §2.9.
- A bar may be shorter than its time signature: the bass's last bar in
  `gp3-b.gp3` is one quarter note in 4/4. It is the
  only such bar in the six binary samples [sample].
- `gp3-a.gp3` sets let ring on 107 notes, `gp5-a.gp5`'s on
  5600: let ring is the norm in some files, and a
  writer should not mark each note.

### 5.3 What the import makes of them

Stage 3 of D166, 2026-11-04: each sample read, written by `lilypond.rs` and
compiled by LilyPond 2.26.0 (`tests/compile_samples.rs`, which fails on any
error or warning), and its first page compared with `inspect_gp.py --beats`.

| File | Tracks | Bars | Compiles | First page checked |
| --- | --- | --- | --- | --- |
| `gp3-a.gp3` | 1 | 25 | cleanly | frets and strings of bars 1–4 against the beats; repeat of bars 1–7; the tempo change to 85 |
| `gpx-a.gpx` | 4 | 17 | cleanly | key F, the melody's frets, a 4-string bass tab, repeats 1–7 and 8–15 (the second without a start), segno and coda, drums as rests |
| `gp3-b.gp3` | 4 | 142 | cleanly | the F5–B♭5 power chords on strings 6–4, dead notes, the drums' grace notes |
| `gp4-a.gp4` | 6 | 100 | cleanly | piano and strings as staves alone, guitars and bass with tab |
| `gp5-b.gp5` | 4 | 80 | cleanly | the Russian title and section names; bar 1's ten beats, ghost note and pull-off |
| `gp5-c.gp5` | 7 | 127 | cleanly | the intro's A minor arpeggio, three vocal and keyboard tracks without tab, the bass's line |
| `gp5-a.gp5` | 10 | 139 | cleanly | C♯ standard and drop B tunings, duplicate names (`acousticOneB`), strokes as arpeggios, set at staff size 14 |

What the first compiles found, each fixed where it arose (D166):

- `\bendAfter #+3\mp` does not compile: Guile reads `+3\mp` as one symbol.
  `gp3-b.gp3`, `gp5-c.gp5` and `gp5-a.gp5` failed on it.
- `gp5-a.gp5`'s first system (17 staves) is taller than an A4 page at size 20.
- An instrument name over about 12 characters (*Distortion Guitar*, a player's
  name) runs past the page's left edge.
- The GPX's rhythm track gives a chord on every beat, so its name was
  printed on every beat; `gp5-b.gp5`'s ghost notes are mf among f notes, which
  printed `mf f mf f`.
- `gp3-a.gp3` leaves the key at C (0), where the GPX has F:
  it is the file's value (`inspect_gp.py` reads 0 too), not a misread.

Still as the files say, and noisy: `gp3-a.gp3` sets let ring on its
chords and not on the notes between, so *let ring* is printed at each chord.

## 6. Sources

- The samples of §1 and `spike/inspect_gp.py`, whose walk is §2's checks.
- Public prose descriptions of the GP3, GP4 and GP5 layouts and of the GPX
  container and GPIF schema, for what is marked [described]. No project's
  code was copied or ported; the readers are written from this note.
