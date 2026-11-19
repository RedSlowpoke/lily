# GarageBand projects from Lily Studio

Whether Lily Studio can hand a score to GarageBand: as a `.band` project it
writes itself, or through a route GarageBand already imports (a MIDI file,
audio files, a Logic project, …). This note starts with what the studio
already produces that such a project could hold (§1), maps the structure of
a `.band` project (§2), and compares the routes into GarageBand that need no
`.band` writer (§3), tests the route of MIDI and stems in GarageBand (§4),
and patches a `.band` that GarageBand saved to see what survives (§5). It
ends with a recommendation (§6), its risks (§7) and a staged plan for the
studio, the Mac app and the web (§8). D162 in `docs/DECISIONS.md` sets out
the route. It supersedes D161, which was written before §9 to §13. Stages 1
to 3 of §8 are built (D162 accepted, D163 to D165), and §8.1 checks their
export in GarageBand.

§9 to §13 then ask whether the studio could write a `.band` after all.
§9 surveys the public code that reads or writes the format, and §9.6 sets
out how LogicProFormatWriter links the records it writes. §10 has projects
GarageBand saved one change apart and a tool that diffs them record by
record. §11 decodes how the records link, checked on every project at hand.
§12 finds that GarageBand opens none of LogicProFormatWriter's projects.
§13 builds projects of a new shape from one GarageBand saved, and GarageBand
opens them. §9 begins with what the five sections found, and §6 to §8
rest on them.

**[measured]** marks what was run on 2026-11-03 on the development Mac with GNU
LilyPond 2.26.0: a score compiled with `lilypond -s` and its MIDI file read
event by event (Appendix A). Every other claim about the studio comes from
its code and the decisions it cites.

## 1. What the studio already has

### 1.1 In short

| What | Where it comes from | Mac app | Web | What a GarageBand project would do with it |
| --- | --- | --- | --- | --- |
| **The compiled MIDI** | lilypond's `\midi`, the last compile's first MIDI file (D44, D54) | yes | yes | a software-instrument track per part |
| **Part names** | `partsOf`/`partNames` (`playbackSetup.ts`) from the playback map's staves | yes | yes | the tracks' names |
| **Instruments** | the MIDI's program changes; the Parts fold's General MIDI choice (D45) | yes | yes | each track's instrument patch |
| **Volumes and mutes** | `playback.json`, per part and for the audio track (D45, D121, D155) | yes | yes | each track's volume and mute |
| **The tempo map** | the MIDI's control track; `atTempo` for `% BPM:` (D155) | yes | yes | the project tempo and its changes |
| **Meter, key, bar 1** | the MIDI's time and key signatures; the playback map's bars | yes | yes | the project's time and key signature; where bar 1 is |
| **The stems** | karaoke-mlx (D112, D125): 2 or 6 stems of a song | yes | no | an audio track per stem |
| **The audio track** | `% Audio:`, `% Audio offset:`, `% BPM:` (D155), found by D159 | yes | no | an audio track placed against the MIDI |
| **The song's grid** | lily-transcribe's beats and `barTimes` (D113, D135, D159) | yes | no | a tempo map that follows a drifting recording |
| **The parts as audio** | `video.ts` renders parts on an `OfflineAudioContext` to a WAV (D140, D155) | yes | not used | a bounce, if a track cannot be MIDI |
| **Words, title, start marks** | the playback map's lyrics (D133); `\header`; `% Start:` (D149), start bar (D45) | yes | partly | probably nothing (§1.8) |

The web build (D55) has the score, its MIDI and the Parts fold, and saves
Export MIDI to the downloads (`src/web/transport.ts`). It has no stems and no
audio track: `src/web/separation.ts` refuses every separation command,
`detect_tempo` among them, with *Separating stems needs the Mac app.*, and
D155 gives the web no audio track because it plays no file of the user's.
GarageBand is a Mac and iOS app either way, so a web export could only be
files to download, such as a MIDI file or a zip.

### 1.2 The compiled MIDI

Lily Studio plays the MIDI file lilypond writes for a score with `\midi`
(D35). *Add MIDI* (D44, `midiBlock.ts`) puts the block into a score that
lacks one; without it there is no MIDI and nothing below except the stems.
Export MIDI (D54, `crates/session/src/midi_export.rs`, shared by the Mac
app's command and the web's worker) saves the bytes of the last compile's
first MIDI file. Its only change is to leave out the muted parts' `MTrk`
chunks. Every other byte stays as lilypond wrote it.

What lilypond 2.26 writes **[measured]**, for a score with a flute staff
(`midiInstrument = "flute"`, `\tempo 4 = 100`, a `\partial 4`, 3/4, G major,
a second `\tempo 4 = 80`), a piano staff of two staves without
`midiInstrument`, and a drum staff:

- **Format 1, 384 ticks a quarter.** Track 0 is a control track with no notes.
  Its name is the `\header`'s title (`Test`), followed by two text events
  (`creator:`, `LilyPond 2.26.0`), the time signature, and each `\tempo` at
  its tick (100 at 0, 80 at 1536). A score without `\tempo` gets ♩ = 60.
- **A track per staff**, named `<staff id>:<voice id>`: `melody:`, `rh:`,
  `lh:`, and `:v` for an unnamed staff with a voice `v`. The drum staff's
  track has no name. `instrumentName` is **not** in the file.
- **Channels** in staff order (0, 1, 2), drums on channel 9.
- **Instruments only where the score names one.** The flute's track has
  program 73, written twice, and an instrument-name meta event (`0x04`)
  `flute`. The drums get program 0 on channel 9 and `drums`. The piano's
  tracks have no program change and no instrument name, so they play
  program 0, the acoustic grand.
- **The key signature** (`0x59`) is on the track of the staff where `\key`
  was written, not in the control track.
- **Words** (`\addlyrics`) are a track of their own: lyric events (`0x05`),
  one per syllable at its note's tick, and no notes. `partsOf` counts no part
  for it, since it has no notes.
- **A pickup is not marked.** The 3/4 time signature is at tick 0, and bar 1
  starts after the `\partial 4`, at tick 384. A program that counts bars
  from tick 0 puts every bar line a quarter early. The studio knows where
  bar 1 is from the playback map (`barOneTime` in `tempo.ts`, D159). The MIDI
  file does not say.

### 1.3 Part names and instruments

- **Names.** The studio does not show the MIDI's track names. `partsOf`
  pairs each track with notes with its staff in the playback map
  (`TimedStaff` in `packages/common/src/types.ts`: the id, `instrumentName`
  and `shortInstrumentName` as plain text, the group such as `PianoStaff`,
  the clef, the voices). `partNames` then names it, in this order: by its
  `instrumentName`; by the choir voices it is named after (*S.A*, *Tenor*);
  by its instrument and clef in a piano staff (*Piano · G clef*); by its own
  id (*Violin one*, *Melody*); or by its instrument. Names that are still the
  same get a number. These are better track names than the MIDI's
  `melody:`, and they exist only in the studio.
- **Instruments.** A part's *written* instrument is the MIDI's programs as
  lilypond names them (`trackInstruments`), *acoustic grand* without one. A
  drum part is one whose channels are all 9. The Parts fold can play a part
  on another General MIDI program from a short list by family
  (`INSTRUMENT_GROUPS`). That choice is kept in `playback.json` (D45) and
  played by `mixMidi`, but **not written into Export MIDI** (D54's *Not
  done*): it would mean rewriting events, not dropping chunks.
- **Volume and mute.** Each part has a mute and a volume from 0 to 200 %
  (D121), and the audio track a mute, a volume from 0 to 100 % and where
  playback ends (D155). All of them are in `playback.json`, and none is in
  any exported file. A GarageBand track has a volume fader and a mute, so
  these could become the tracks' starting mix.

### 1.4 The tempo map, the meter and bar 1

- **The tempo map** is in the control track (§1.2). midi.js parses it into
  `tempos`, entries of `{ tick, seconds, tempo }`, the tempo in µs a
  quarter. `scoreBpm` reads the first one. `bpmAt` (`tempo.ts`) gives the
  tempo at any time.
- **`% BPM:` (D155)** plays the music at another tempo to keep it with the
  audio track. `atTempo` scales every time and every tempo by the score's
  first tempo over `% BPM:`, so a later `\tempo` keeps its proportion. The
  ticks do not move. A MIDI file at the recording's tempo therefore needs
  only the control track's tempo events (`0x51`) rewritten, each by that
  same ratio. Export MIDI does not do that yet: it writes the score's own
  tempo (D155's *Not done*). A tempo of 113.76 is 527 426 µs a quarter, so
  the file can keep its hundredths. Whether GarageBand keeps them is for §2.
- **A drifting song** made into a score by lily-transcribe gets `songTempo`,
  a hidden `\tempo` at every bar from the recording's `barTimes` (D135). Its
  drift is already in the MIDI's tempo map. A hand-written score with a
  drifting recording has one tempo and a warning (D159). Its bar map,
  `% Audio bars:`, is not built.
- **The meter.** The MIDI has every time signature (`0x58`). The studio
  reads only the first one (`timeSignatureOf`, `scoreMeter` in `tempo.ts`),
  for D159.
- **Bar 1 and the bars.** The playback map has every bar, with its number
  and its moment (`TimedBar`). `barOneTime` gives when bar 1 sounds: 0
  without a pickup, the pickup's length with one. A GarageBand project whose
  bar 1 should be the score's bar 1 needs this, since the MIDI file does not
  carry it (§1.2).

### 1.5 The stems from karaoke-mlx

- **What is written.** karaoke-mlx (`packages/separate`) writes one file
  per stem, `<song> - <stem>.<format>`, into a folder `<song> (karaoke)` or
  `<song> (stems)` beside the song (`cli.py`). The karaoke models
  (`becruily`, `aufr33-viperx`) write `vocals` and `instrumental`.
  `bs-roformer-sw` writes `vocals`, `drums`, `bass`, `guitar`, `piano` and
  `other`. *Mix* sums ticked stems into one file.
- **Format.** 44.1 kHz stereo. The user chooses WAV (16-bit PCM), FLAC or
  MP3 at 320 kbit/s (`audio.py`, `FORMATS` in `separate.ts`). There is no
  AIFF output.
- **Alignment.** Each stem is as long as the song and starts at its sample 0.
  `separate.py` reflect-pads the song for its chunks and trims the padding
  off. The stems therefore line up with one another and with the song, and
  could go into GarageBand at one common start. An MP3 adds its encoder's
  delay. WAV is the safe choice, and FLAC is safe only if GarageBand reads
  it (§3).
- **What the studio knows of them.** Past separations are in
  `separations.json` (D114), with their folder and their stems. A score made
  from a stem names its files in its own lines (D112, D133, D137): the stem
  it was written from in `% Stem:`, the backing track to sing over in
  `% Backing:`, the music without any voice in `% Music:`. All of them are
  beside the score.
- **Mac only.** Separation runs uv and MLX on the user's Mac
  (`web/separation.ts`).

### 1.6 The audio track: its offset and tempo

- **The lines (D155).** `% Audio: <file beside the score>` names the
  recording. `% Audio offset:` is in seconds to the hundredth, and the
  recording's place is the music's place less it. A positive offset makes
  the recording wait until the music reaches it. A negative one means the
  recording is that far in at the music's 0. `% BPM:` is in quarter notes a
  minute to the hundredth. These lines are in the score, so they go with it.
  The track's volume, mute and end are in `playback.json`.
- **Found from the recording (D159).** *From the recording* runs
  `lily-transcribe tempo` on the `% Audio:` file. It writes `% BPM:` (the
  tempo found, at the score's metrical level, with its hundredths) and
  `% Audio offset:` (the score's bar 1 at the new tempo less the recording's
  first bar line, `barOne`). `tempo.ts` turns the answer into the lines.
- **What a project needs from it.** The two values say where the recording
  goes against the MIDI. With a positive offset, the audio region would
  start that many seconds after the music's start. With a negative one, it
  would start at the music's start and be cut that far into its file, or
  the MIDI would be moved that far later. The project's tempo would be
  `% BPM:`, since that is the tempo the MIDI plays at against the
  recording. How GarageBand places a region in seconds rather than in bars
  is for §2 and §3.
- **Mac only**, as D155 has it. There is also no audio track without `\midi`.

### 1.7 The parts as audio

`video.ts` renders the score's MIDI on an `OfflineAudioContext`, through
midi.js's `Synth`, at the parts' volumes. It writes a 16-bit stereo WAV at
48 kHz (`wav()`), and Export Video gives it to ffmpeg with the recordings.
The same code could bounce one part, or all of them, to a WAV beside the
stems. That is a fallback for a route that takes only audio. The sound would
be the studio's General MIDI synth, not a GarageBand instrument, and the
WAV would be at 48 kHz while the stems are at 44.1 kHz.

### 1.8 What the studio does not have, which an export would need

- **No `.band` writer**, and no knowledge of the format: §2.
- **No MIDI writer** past dropping chunks. Several things are not in the MIDI
  file today: the part names, the Parts fold's instruments, the volumes, a
  tempo at `% BPM:`, and a mark for where bar 1 is after a pickup. Each would
  mean rewriting or adding events. The track-name and tempo events are the
  simplest of these.
- **No AIFF.** karaoke-mlx writes WAV, FLAC and MP3. ffmpeg, which it
  already runs, writes AIFF too.
- **The words** have no obvious place: GarageBand shows no lyrics as far as
  is known. Whether a MIDI file's lyric events survive an import is for §3.
- **The start marks.** The start bar (D45) and `% Start:` (D149) could at
  most be the playhead or a cycle region.

### 1.9 Questions for the next sections

- **§2, the `.band` project.** Which of its files hold the track names, the
  instruments, the volumes and mutes, the tempo and its changes, the time
  and key signatures, and an audio region's position and trim? Which of them
  are readable, and which are in the binary `ProjectData`?
- **§3, the import routes.** When GarageBand imports a multi-track MIDI
  file, does it keep the track names, the program changes (or play
  everything on a piano), the tempo changes with their hundredths, the time
  and key signatures, and the lyric events? Does it read FLAC? Does a stem
  dropped in land on bar 1 or where it was dropped?
- **§4, the MIDI-and-stems spike.** Does a pickup shift the bars? Does a
  negative audio offset survive as a trimmed region?

## 2. The structure of a `.band` project

### 2.1 What was taken apart

- **GarageBand 10.4.14** (6648) is installed, on macOS 26.5.2. There is no
  Logic Pro.
- **Four projects saved by hand** with 10.4.14 **[measured]**:
  - **empty**: Empty Project, one software-instrument track, nothing on it.
  - **midi**: `lily-test.mid` opened in GarageBand and saved. The file is a
    format-1 MIDI file at 480 a quarter with one track, *Flute Part*,
    program 73 and twelve quarter notes; its control track has 3/4, ♩ = 100
    at bar 1 and ♩ = 80 at bar 3.
  - **tempo**: an empty project set to 90, 3/4 and G major, with a point
    drawn in the tempo track at bar 5, down to 72.
  - **audio**: an empty project with one audio track, and `lily-click.wav`
    (4 s, 44.1 kHz, 16-bit stereo) dragged to bar 2.

  GarageBand could not make them from the terminal. UI scripting is denied
  (`osascript is not allowed assistive access`). Its AppleScript is the
  bare standard suite: it names windows, and cannot reach documents. A
  first launch also stops at an *Essential Sounds Download* dialog. The
  user made them by hand, following the steps above.
- **The six templates in the app** (`Contents/Resources/Project
  Templates/NewProject/*.band`) **[measured]**:
  - Keyboard Collection, Amp Collection, Voice, Hip Hop, Electronic and
    Songwriter, saved by *GarageBand 10.2.0 (4769.4/5)*.
  - 5 to 9 tracks of instruments, amps, vocal strips and a Drummer region,
    at 70, 96, 120 or 128 in 4/4.

  They show an older format next to the new one (§2.7).
- **The tool.** `spike/inspect_band.py` is read-only and uses only the
  standard library. It prints a project's files and plists and walks its
  `ProjectData`. `--chunks` lists every record, `--dump MSeq` dumps one tag
  in hex, and `--diff A B` pairs two projects' records and says which
  differ. Run it with `python3 -I`. It reads every file of all ten projects
  from start to end. `spike/diff_band.py` (§10.2) diffs two projects record
  by record, and `--links` prints and checks how the records link (§11).
  `spike/wrap_projectdata.py` (§12.1) puts a `ProjectData` into a package,
  and `spike/build_band.py` (§13.1) adds tracks, regions and files to one.
  [`spike/README.md`](spike/README.md) lists them all.

### 2.2 The package

A `.band` is a directory that Finder shows as one file (`LSTypeIsPackage`,
UTI `com.apple.garageband.project`). The audio project, as 10.4.14 saved it:

```
audio.band/
  projectData                          169 544  XML plist: the GarageBand for iOS document, a fixed stub (§2.4)
  Alternatives/000/
    ProjectData                        259 769  the song, binary (§2.5)
    MetaData.plist                         668  a readable summary of the song (§2.3)
    DisplayState.plist                   2 493  window layout, screensets, editors' state
    DisplayStateArchive                  4 516  the same as an NSKeyedArchiver
    WindowImage.jpg                    147 859  a screenshot of the window, for the file's preview
  Media/Audio Files/lily-click.wav     707 040  the dropped WAV, copied in
  Resources/ProjectInformation.plist      210  saved-by version, alternative names
```

- **The four saved projects have this layout and nothing more.** empty, midi
  and tempo lack only `Media/`. The ProjectData of an empty project is
  243 393 bytes, most of it the channel strips' plug-in state.
- **The templates differ.** They have no `WindowImage.jpg`. They have a
  `Contents/` with `PkgInfo` (`BNDLband`), `Resources/template.tif` and 37
  `.lproj/InfoPlist.strings` for the template chooser
  (`CFBundleDisplayName`, `TemplateDescription`). A saved project has no
  `Contents/` at all.
- **`Alternatives/NNN/`** holds one alternative (Logic's *project
  alternatives*). Only `000` exists in all ten. `VariantNames` in
  `ProjectInformation.plist` names each one by its number. The
  MultimediaWiki page (§2.7) adds `Autosave/`, `Previews/` and
  `Undo Data.nosync/` beside it. None of the ten has them.
- **`Media/Audio Files/`** gets a dropped audio file. *Copy audio files into
  project* is on by default. The copy keeps its name, and its samples are
  byte for byte the original's. GarageBand appends a 1388-byte `LGWV`
  chunk to the end of the WAV, likely Logic's waveform overview, so the
  file grows from 705 644 to 707 040 bytes **[measured]**.
- **There is no manifest of the package's files.** `MetaData.plist` lists the
  audio files, and the song has its own record of each (`AuFl`, §2.6).

### 2.3 The plists: readable, and secondary

- **`Resources/ProjectInformation.plist`** has `BundleVersion` 2,
  `LastSavedFrom` (*GarageBand 10.4.14 (6648)*) and `projectAssetFlags`. The
  flags are 8409 in the saved projects and 8413 in the templates. They are
  probably the *copy into project* choices of Logic's File › Project
  Settings › Assets, but that is unverified. `VariantNames` holds the
  project's name (`{"0": "audio"}`), and `VariantNamesV2` holds
  `{PROJECT_NAME}`.
- **`Alternatives/000/MetaData.plist`** is plain and readable:
  - `BeatsPerMinute`, `SongSignatureNumerator` and
    `SongSignatureDenominator`, `SongKey` and `SongGenderKey`, and
    `SignatureKey`.
  - `SampleRate`, `NumberOfTracks`, `isTimeCodeBased`, `FrameRateIndex`
    and the surround indexes.
  - The files the song uses:
    - `AudioFiles` and `UnusedAudioFiles`, relative to `Media/`: audio
      has `["Audio Files/lily-click.wav"]`.
    - `PlaybackFiles`.
    - `SamplerInstrumentsFiles`, `ImpulsResponsesFiles` (*sic*),
      `QuicksamplerFiles` and `UltrabeatFiles`, by absolute path into
      `/Library/Application Support/Logic/…` and the app.

  It holds only the *first* tempo and meter. midi has 100 and 3/4, and
  tempo has 90, 3/4 and G. `SignatureKey` is 7 for C major and 8 for G
  major, the same value as the key event in `ProjectData`. Everything in it
  is also in `ProjectData` (§2.6), so it looks like a summary written for
  Finder, the Quick Look and thumbnail extensions, and project management.
  GarageBand does not read it on open (§5.3).
- **`DisplayState.plist` and `DisplayStateArchive`** hold window frames,
  screensets, the transport layout, which editors and panes are open, and
  zoom. None of it is musical. A writer could copy them from a template
  unchanged.

### 2.4 The top-level `projectData`: a fixed stub for GarageBand for iOS

`projectData` is an XML plist of 169 544 bytes. It is the same file, with
one MD5, in all six templates *and* all four projects saved by 10.4.14
**[measured]**. GarageBand on the Mac writes a constant file there and never
updates it. It is an `NSKeyedArchiver` whose `$top` holds:

- `DfDocument logic model` (class `DfLogicModel`), whose
  `DfLogicModelLogicSong` is an `NSMutableData` of 114 302 bytes. These are
  a complete song in the chunk format of §2.5, at format `0x06a6`: 110 bpm,
  *Untitled* sequences, no tracks of the project's own.
- `DfDocument arrange model` (`DfArrangeModel`): grid, snap, quantise, zoom,
  metronome and count-in settings, and podcast keys. Its title is *My Song
  38*.
- `DfDocumentSystem` `macos`, `DfDocumentMachine` `x86_64`, `Version` 55000,
  `CbVersion` 30000.

The `Df` classes are GarageBand for iOS's document classes. A writer can copy
this file unchanged. What GarageBand for iOS makes of a Mac `.band` is for
§3. The MultimediaWiki page describes the `.band` of GarageBand before 10 the
same way: an XML `projectData` holding the song as a blob.

### 2.5 `ProjectData`: a flat run of records

The format is Logic Pro's (`.logicx` has the same `Alternatives/NNN/ProjectData`).
It is little-endian. All ten projects and the iOS stub walk from byte 24 to
the last byte with no gap **[measured]**.

**File header**, 24 bytes:

| Offset | Bytes | Meaning |
| --- | --- | --- |
| +0x00 | `23 47 c0 ab` | magic, `#G\xC0\xAB` |
| +0x04 | `d0 09` (10.4.14), `cb 09` (10.2.0's templates), `a6 06` (the iOS stub) | format code |
| +0x06 | `03 00 04 00 00 00 01 00 08 00` | the same in every file seen, and in Logic 11 (§2.7) |
| +0x10 | u64 | length of the rest of the file, = size − 24 |

**Record header**, 36 bytes, followed by the record's body. The field names
are those of LogicProFormatWriter's spec (§2.7).

| Offset | Field | Seen |
| --- | --- | --- |
| +0x00 | tag, a four-character code stored reversed | `gnoS` = Song, `qeSM` = MSeq, … |
| +0x04 | u16 schema version, per tag | Song 3, MSeq 3 (5 in 10.4.14), Trak 5, EvSq 1, Envi 6, AuFl 4, AuRg 3 |
| +0x06 | u16 class id: a role, for MSeq and EvSq | EvSq: 1 = meter and key, 3 = tempo, 22 = arrange markers, 23 = a track's or region's events, 25 = ? |
| +0x08 | u32: the object index, shifted left by 18 | Envi 0 to 46, MSeq 0 to 256 |
| +0x0C…+0x15 | ordinal and link fields | `ffff…` or small numbers |
| +0x16 | u16 2, u16 0 | the same in every record |
| +0x1A | u16 header form | **2 in every record 10.4.14 wrote, 1 in the templates** |
| +0x1C | u64 length of the body | record = 36 + length |

**Tags.** These are the counts in empty, then in the Voice template, with
what each holds:

| Tag | n | Holds |
| --- | --- | --- |
| `Song` | 1, 1 | the song's root: 10 996 bytes in 10.4.14, 3 716 in the templates |
| `MSeq` | 15, 24 | a sequence: the arrange, *TRASH*, *Track Automation Root Folder*, *Track Alternatives*, *Global Harmonies*, *\*Automation*, and one per MIDI region (named after the MIDI track: *Flute Part*) |
| `EvSq` | 15, 24 | the events of a sequence: tempo, meter and key, notes, program changes, regions' places, automation (§2.6) |
| `Trak` | 28, 52 | a row of a sequence's track list (§11.1): 58 bytes of body in 10.4.14, 57 in the templates, and an empty one that ends each list |
| `Envi` | 16, 23 | environment objects: inputs, click, buses, master, and a channel strip per track named after its patch (*Classic Electric Piano*, *Flute Solo*) or its audio file (*lily-click*), and one per reverb send |
| `AuFl` | 0, 0 | an audio file (audio has 1) |
| `AuRg` | 0, 0 | an audio region (audio has 1) |
| `AuCn`, `AuCO`, `AuCU` | 13/283/38, 13/879/128 | channel strips, plug-in slots and their state |
| `Hypr` | 3, 3 | hyper sets (MIDI controller and GM drum-name lanes) |
| `Layr`, `InSt`, `ScSt`, `TxSt`, `Styl`, `Vide` | 1, 1, 1, 32, 32, 1 | environment layers, the score's instrument set, score text styles, video settings |
| `SngO` | 2, 2 | song options; one embeds an `NSKeyedArchiver` bplist (Drummer state) |
| `GenM`, `CorM` | 1–2, 2 | the Drummer/Session Player generator, and unknown |

Opening a MIDI file adds a *GM Device* and sixteen *@ (=Context Name)*
objects to the environment: midi has 31 `Envi` records against 16.

### 2.6 What can be read and what is opaque

An event sequence (`EvSq`) is a run of 16-byte records ending in a record
whose first byte is `0xf1` at position `0x3fffffff`. An event is a head
record and the records after it whose byte 7 has its top bit set. A head's
bytes 4 to 7 are its position, always below 2³¹. Positions are in ticks of
**960 a quarter**. Tempo, meter and the events in a region count from bar 1
at **38400** (`0x9600`). A region's place on its track counts from **34560**,
one 4/4 bar earlier. All of the following is **[measured]** on the four
projects unless it says otherwise.

| What | Where and how | Readable? |
| --- | --- | --- |
| **Tempo map** | EvSq class 3, `0x60` events: the position, then bpm × 10000 as a u32 in the next record. empty: 120 at bar 1. midi: 100 at bar 1, 80 at q+6 = bar 3 in 3/4. tempo: 90 at bar 1, 72 at q+12 = bar 5. An imported tempo is 2 records, one drawn in the tempo track 4 (its curve). The hundredths fit: 113.76 is 1137600 | yes |
| **Tempo, again** | `Song` +110 and +114, the same unit, but neither follows the map: tempo has 72 and 120 for a map of 90 then 72, midi 80 and 100, Songwriter 110 and 96 | no; the tempo is the EvSq |
| **Meter** | EvSq class 1, a `0x30` event of 3 records. Byte 11 is the denominator's exponent (2 = quarter) and byte 12 the numerator: `02 04` in 4/4, `02 03` in 3/4. Its other fields put bar 1 a whole number of bars after 0: −10 bars at 3840 in 4/4, and −13 bars plus 960 ticks at 2880 in 3/4 (38400 = 13 × 2880 + 960) | yes |
| **Key** | EvSq class 1, a `0x32` event, byte 12: 7 for C major, 8 for G major, equal to `MetaData`'s `SignatureKey`; so probably 7 plus the sharps (7 minus the flats). Minor keys not tried | the major keys tried |
| **Notes** | EvSq class 23 in the region's MSeq, `0x9n` events (n = channel), 2 to 3 records: velocity at byte 11, pitch at 12, length in ticks at +12 of the next record. midi's twelve quarters read back as pitches 67, 69, 71, … at 960 ticks; the file's 480 a quarter were rescaled | yes |
| **Program** | a `0xCn` event at the region's start, program at byte 11: 73 for the flute | yes |
| **Track and region names** | a u16 length and UTF-8 in MSeq (+0x10) and Envi. midi's region MSeq takes the MIDI track's name (*Flute Part*). Its channel strip takes the patch GarageBand chose for program 73 (*Flute Solo*). The audio track's strip takes the file's name (*lily-click*). Names are variable-length: the arrange MSeq is 303 bytes named *empty* and 297 unnamed | yes; a rename changes record sizes |
| **Instruments** | the patch's name in the channel strip's `Envi`; its sound in `AuCO`/`AuCU` plug-in state and `MetaData`'s sampler files | names yes, sound no |
| **Audio region's place** | EvSq class 23 of the track, a `0x24` event of 5 records (80 bytes) at 38400 from the region origin 34560, i.e. bar 2 in 4/4, where it was dropped | the position yes |
| **Audio region** | `AuRg`: its name (*lily-click*, u16 length at +0x4A) and its length in frames (176 400 at +0x16), and two 16-byte ids | name and length yes |
| **Audio file** | `AuFl`, 616 bytes: the file name in UTF-16 (`0e 00` then *lily-click.wav*), `LFUA`, and the **absolute path of the folder** (`/Users/…/audio.band/Media/Audio Files` at +0xB0). Then its size (707 040), `EVAW`, its frames (176 400), the rate (44 100), channels (2) and bits (16) | yes |
| **Volume, pan, mute** | the channel strip's `AuCO`. daw2logic writes the volume as a float at `OCuA` +0x98, and wavelength and logic2ableton read the fader, pan, mute and solo there (§9.2, §9.4) | not checked in GarageBand's files |
| **Links between records** | **[measured]** on 31 projects (§11): by index or slot, never by offset. A track row names its channel strip's `Envi`. A region's placement names its row, and the region's `MSeq` or its `AuFl` and `AuRg`. An `Envi` names its `AuCO` by number. The ids in `AuRg` and in the rows are referenced nowhere else | yes |
| **Per-record counters** | **[measured]** (§11.3): *MSeq +8* is a counter at body +0x08, repeated at its `EvSq`'s rec +0x0E. It is distinct in a project and renewed on every save, by a rule not found. New values, counting up from the highest in the project, open (§13). The 12 bytes near each `Envi`'s end are its strip's id (§11.2), whose other 4 bytes stay the same in a session. The arrange's height and `NumberOfTracks` follow the track count; the `Song` fields that LPFW counts with hold the selected track | the links yes; the counter's rule no, and it is not needed |
| **Bookkeeping** | **[measured]** (§13.2): the `Song`'s registry is two tables, of ids and of time stamps, with a row for every object of classes 1 to 33, sorted. `AuCn` 9 counts the strips by kind. A builder that keeps both up gets projects GarageBand opens; whether GarageBand needs them was not tried | yes |
| **Integrity** | no checksum found; the file header's length is the only total | GarageBand checks neither (§5.4) |

In short, the parts of a song Lily Studio would write can be read and are
decoded publicly: the tempo map, meter, key, notes, programs, names, an audio
file and its region's place. §11 has since decoded what holds them
together: how a track, its channel strip, its sequences and its regions
point at one another. §13 keeps those links when it adds tracks, and
GarageBand opens the result. What stays opaque is the plug-in state, the
rule of the sequence counter, a few fields of a region's placement, and the
`Song`'s other tables (§11.6). Two findings bear on any writer:

- **Names are variable-length**, so a renamed track resizes its records.
  The walk tolerates that, because every record carries its length, and no
  record points at an offset (§11.1). The fields after a name start at an
  even offset, so a name that changes from even to odd length needs a pad
  byte (§11.4).
- **`AuFl` holds an absolute path** into the package. A `.band` written
  elsewhere and moved, or a template patched in place, carries a stale path.
  GarageBand ignores it when the file is in `Media/Audio Files/` (§5.3).

A writer should therefore start from a project GarageBand saved.
GarageBand refused every `ProjectData` that Logic or LogicProFormatWriter
wrote (§12). From such a base, a writer can change values in place (§5) and
add tracks, regions and audio files by copying the base's own records with
their links set (§13). It cannot build a track from nothing, since a new
strip's plug-in state is still a copy of one that GarageBand wrote.

### 2.7 Public work, and how the format differs by version

Logic's `ProjectData` is the same format as GarageBand's, and it has been
decoded at length in public. §9 surveys that work, about forty projects
gathered on 2026-11-03 and read first-hand, with their licences. In short:

- **GarageBand's own `ProjectData`.** Two projects read the Mac's:
  [wavelength](https://github.com/austinginder/wavelength) (C++, MIT) reads
  the most of it, and [band-to-midi](https://github.com/zhafen/band-to-midi)
  (Python, MIT) its notes and tempo, both from 10.4.14's saves. Only
  [gb-in-Claude](https://github.com/rafaqat/gb-in-Claude)
  (TypeScript, MIT) writes into one. It adds audio regions and files to a
  project GarageBand saved, and has GarageBand make each track (§9.2).
- **GarageBand for iOS.** [Band2Midi](https://github.com/tomoutsuki/Band2Midi)
  (GPL-3.0) and [band2flp](https://github.com/veexctrl/band2flp) (MIT) read
  its projects (§9.2).
- **Logic's `ProjectData`.**
  [LogicProFormatWriter](https://github.com/jonkubis/LogicProFormatWriter)
  (Python, MIT) writes it from seeds that Logic saved, and
  [logicxkit](https://github.com/phierceweb/logicxkit) (Python, Apache-2.0)
  adds tracks to it the way Logic does (§9.3). Their authors tried both
  in Logic only. GarageBand opens none of LogicProFormatWriter's output
  (§12).
  Of the readers, [logic2ableton](https://github.com/Evilander/logic2ableton)
  (MIT) walks the records in full (§9.4).
- **Descriptions.** The [MultimediaWiki page](https://wiki.multimedia.cx/index.php/GarageBand)
  (2015) has the `.band` layouts before 10 and at 10.0.3. The [Library of
  Congress's fdd000640](https://loc.gov/preservation/digital/formats/fdd/fdd000640.shtml)
  (2025) describes the `.logicx` package and its `Alternatives` folders.

What they say agrees with the projects above. LogicProFormatWriter's spec
has the same file header, 960 a quarter, the origins of 38400 and 34560,
tempo as bpm × 10000, the meter event, the 80-byte audio placement event
starting `24 00 00 00`, and `lFuA`/`EVAW` in the audio file record. The
links that four readers state independently (§9.5) hold in GarageBand's
files (§11), and so does most of LogicProFormatWriter's model (§11.5).

The GPL projects can be read, but their code cannot come into the MIT apps.
MIT and Apache-2.0 code could come in with its notice, and code with no
licence not at all. §9.5 says which project is which, and §7.2 has the rest
of the licence questions.

**By version:**

- **GarageBand before 10 (to '11, 6.0):** a `.band` with an XML
  `projectData` holding the song as a blob, and `Media/`, `Output/` and
  `Thumbs/` (MultimediaWiki). Not tried.
- **GarageBand 10.0 (2013) on:** Logic's engine and Logic's
  `Alternatives/000/ProjectData`. Layout recorded at 10.0.3.
- **GarageBand 10.2.0** wrote format `0x09cb` with header form 1 (the
  templates), schema version 3 for `MSeq`, 57-byte `Trak` rows, and a
  `Song` of 3 716 bytes.
- **GarageBand 10.4.14** writes **`0x09d0` with header form 2**, `MSeq`
  version 5, 58-byte `Trak` rows and a `Song` of 11 to 12 KB, and no
  `Contents/`.
- **The same codes as Logic.** The fixtures of §9's projects date each
  code (§9.5): `0x09cf` is Logic 11.2.2, `0x09d0` Logic 12.2 and
  `0x09d1` Logic 12.3.1 and 12.4. So GarageBand 10.4.14 writes Logic
  12.2's code, and current Logic writes a newer one. LogicProFormatWriter's
  spec puts `d0 09` under Logic 11.2.2, but its own Logic 11.2.2 fixtures
  have `0x09cf`. A shared code is not enough: GarageBand refuses a project
  that Logic 12.2 saved at `0x09d0` (§12).
- **Old projects open.** 10.4.14 ships the 10.2.0 templates and offers them
  in its chooser, so it reads `0x09cb` and form 1. It writes `0x09d0` and
  form 2 on save. A writer that patches a template inherits the older
  format and lets GarageBand upgrade it. Patching a project 10.4.14 saved
  gives the current format.
- **GarageBand for iOS** has its own document in the top-level `projectData`
  (§2.4), a stub in every Mac project.

### 2.8 What is left for step 5, and a note for §3

- **Which file wins.** Does GarageBand trust `MetaData.plist` or
  `ProjectData` when the two disagree on the tempo or the meter?
- **Moved media.** Does GarageBand find a WAV in `Media/Audio Files/` when
  `AuFl`'s absolute path is stale? Does it need the `LGWV` chunk, or does
  it write one on open?
- **Patching without breaking links.** Can a value be changed in place?
  That means a tempo, a region's place, or a name of the same length. Does
  a longer name, or a changed per-record counter (MSeq +8), stop the
  project from opening?
- **Integrity.** Does GarageBand check anything beyond the header's length?
- **For §3, already seen.** Opening `lily-test.mid` (§2.1) kept:
  - both tempos and the 3/4;
  - the MIDI track's name, as the region's name;
  - program 73, as GarageBand's *Flute Solo* patch;
  - the notes, rescaled from 480 to 960 a quarter.

  Whether a format-1 file with several tracks keeps as much is in §3.3,
  and step 4 tests it.

## 3. Routes into GarageBand that avoid writing `.band`

### 3.1 What was looked at

- **GarageBand 10.4.14 itself** **[measured]**:
  - its `Info.plist`: the document types it opens and the types it declares;
  - its AppleScript dictionary (`sdef`) and its `PlugIns`;
  - its English strings, `Resources/en.lproj/*.strings` converted with
    `plutil`;
  - `Resources/Default Patches/GM_Instrument_Mapping.plist`;
  - two read-only AppleScript queries to the running app.
- **Apple's guides**, fetched on 2026-11-03:
  - [Import audio and MIDI files in GarageBand on Mac](https://support.apple.com/en-au/guide/garageband/gbndd01649ed/mac);
  - the same page for [iPhone and iPad](https://support.apple.com/guide/garageband-iphone/chsab9d208e/ios);
  - Logic Pro's note that its projects do not open in GarageBand
    ([Logic Pro guide](https://help.apple.com/logicpro/mac/10.1/en.lproj/lgcpa8854ca7.html)).
- **Step 2's one-track MIDI project** (§2.8).

Nothing here imported a MIDI file with several tracks, a stem, a MusicXML
file or a Logic project. There is no Logic Pro and no iPad here. A claim
that rests on such a test says *step 4*, and step 4 is where it gets tried.

### 3.2 In short

| Route | Keeps | Loses | The studio would need | Verdict |
| --- | --- | --- | --- | --- |
| **Multi-track MIDI file** (§3.3) | notes, tempo changes, meter, a patch for each program; each track's name as its region's name | where bar 1 is after a pickup; violin and cello (and other close GM programs) merged into one patch; the mix | a MIDI rewriter: names, programs, `% BPM:`, a lead-in for a pickup | **the base of any route**. Works on the web too |
| **AIFF/WAV stems** (§3.4) | the audio exactly; the file's name as the track's name | the region's place (it lands where it is dropped); FLAC | stems padded or cut so sample 0 is the MIDI's tick 0; names by part | **works**, with one drop by hand at bar 1 |
| **MIDI and stems together** (§3.5) | both of the above in one project | the mix, mutes, track order | both of the above, in one folder | **the route to test in step 4** |
| **AppleScript, Shortcuts** (§3.6) | can open a file, and perhaps save it as `.band` | there are no tracks, regions or tempo to script | Apple Events permission | **at most a convenience**: `open -a` |
| **Logic Pro `.logicx`** (§3.7) | everything a writer puts in | GarageBand cannot open it | a `ProjectData` writer, as for `.band` | **not a route into GarageBand** |
| **MusicXML** (§3.8) | notes, parts, perhaps notation | no known way into GarageBand; no audio | a MusicXML writer, which neither LilyPond nor the studio has | **no** |
| **GarageBand for iOS** (§3.9) | MIDI tracks (up to 32), AIFF/WAV/MP3 | drums as Keyboard tracks; tempo map undocumented | the same files, through Files or iCloud Drive | **the same files as the Mac**, less certain |

### 3.3 A multi-track MIDI file

- **What GarageBand takes.** `public.midi-audio` is a document type, with
  `mid`, `midi`, `smf` and `kar`, in the role *Viewer* **[measured]**. A
  MIDI file gets in two ways:
  - **Opened** (File › Open, a double-click, `open -a GarageBand`): a new
    project. Step 2's `lily-test.mid` came in this way and kept its tempo
    map.
  - **Dragged** onto a software-instrument track or the empty area below the
    tracks. Apple says *the MIDI file appears on one or more software
    instrument tracks*. Into an existing project, GarageBand asks *Also
    import tempo information? This will replace the project's current tempo
    information in the range of the MIDI file* **[measured]**, from its
    strings. It warns that recorded audio will then be out of sync.
- **What arrived from one track** (step 2, §2.8) **[measured]**: both tempos
  and the 3/4, the notes (rescaled from 480 to 960 a quarter), program 73 as
  *Flute Solo*, and the MIDI track's name, as the *region's* name. The
  track's channel strip took the patch's name. Which of the two names the
  track header shows is for step 4.
- **The instrument mapping is a fixed table** **[measured]**.
  `GM_Instrument_Mapping.plist` maps each program 0 to 127 (0-based, as in
  the file) to a GarageBand patch. The 128 programs map to only 72 patches:
  - 0 to 3 are all *Steinway Grand Piano*;
  - 40 to 45 are all *String Ensemble*: violin, viola, cello, contrabass,
    tremolo and pizzicato strings;
  - 64 to 67 (the saxophones) are one *Saxophone*;
  - 72 to 79 (piccolo to ocarina) are *Flute Solo* except 75 and 76.

  A string quartet arrives as four String Ensembles, and its parts can then
  be told apart only by name. Keys 128 to 137 are drum kits (*SoCal*,
  *Portland*, *Retro Rock*, *Roots*, *Epic Electro GB*, *Boutique 808 GB*,
  *Orchestral Kit*). Which of them a channel-10 track gets, such as
  lilypond's drums with program 0, is for step 4. Logic has its own table
  beside it (`GM_Instrument_Mapping_Logic.plist`: program 74 is *Medieval
  Recorder* there, *Flute Solo* in GarageBand). The patches play only once
  the sound library is installed: a first launch asks for the *Essential
  Sounds* (§2.1).
- **A track without a program change** may not get a piano.
  GarageBand's default software-instrument patch is *Classic Electric Piano*
  (`DefaultPatchInfo_GarageBand.plist`), and lilypond writes no program for
  a part without `midiInstrument` (§1.2), so the piano parts of Appendix A
  might arrive on an electric piano. Step 4 checks this. Either way, an
  export should write a program change on every track.
- **Lost for certain:**
  - **Where bar 1 is after a pickup.** Tick 0 is GarageBand's bar 1 (step
    2's first tempo is at bar 1), and lilypond does not mark the pickup
    (§1.2). A `\partial 4` puts every bar line a quarter early. The fix is a
    lead-in: move every event later by a bar less the pickup. The pickup
    then ends bar 1, and the score's bar 1 is GarageBand's bar 2.
  - **What the studio has and the file lacks** (§1.8): the part names, the
    Parts fold's instruments, the volumes, and `% BPM:`. They reach
    GarageBand only if the studio writes them into the file.
  - **The mix.** Mutes reach it as missing tracks, since Export MIDI already
    drops muted parts.
- **Not known, for step 4:**
  - whether each of several tracks becomes its own track;
  - what the track headers are named;
  - the key signature, which sits on a staff's track (§1.2);
  - a tempo's hundredths (113.76);
  - lyric events, and the control track's name (the title);
  - volume and pan as CC 7 and CC 10;
  - whether a file can hit *Data conflict in MIDI File. Only n of m tracks
    accepted*. That is a message of Logic's importer, which ships in
    GarageBand's strings.
- **What the studio would need.** A rewriter of lilypond's MIDI, not a new
  format:
  - a track-name event (`0x03`) from `partNames`;
  - a program change on every track, from the Parts fold;
  - the tempo events scaled to `% BPM:` (§1.4);
  - the lead-in for a pickup;
  - perhaps CC 7 from the volumes.

  It runs on the playback map and the MIDI bytes, which the web build has
  too, so the web build could offer the file as a download.

### 3.4 AIFF and WAV stems dragged in

- **Formats.** AIFF, CAF and WAV are document types in the role *Editor*
  **[measured]**. Apple's guide adds AAC, Apple Lossless and MP3 by drag.
  **FLAC** is neither a document type nor in Apple's list, and no string
  names it. Dragged in, it is refused (§4.4). A stem in FLAC must be converted, and
  karaoke-mlx's WAV is the one to hand over. MP3 adds its encoder's delay
  (§1.5).
- **Where it lands.** Where it is dropped: step 2's WAV, dropped at bar 2,
  sits exactly at bar 2 **[measured]**. No file says where a region goes.
  The user drops at bar 1, which the grid's snap makes easy.
- **Its name.** The track's channel strip takes the file's name (*lily-click*,
  step 2). Stems named after their parts (`Vocals.wav`, `Bass.wav`) name
  their tracks. karaoke-mlx's `<song> - vocals.wav` would name a track
  *Song - vocals*.
- **Tempo.** Apple's iOS guide says that only Apple Loops follow tempo
  changes. On the Mac a plain file is not stretched either, as far as is
  known (step 4). That is what the route needs: the project's grid comes from
  the MIDI at `% BPM:` (§3.3), and the recording stays as it was. A drifting
  song's grid follows it through `songTempo`'s tempo map (§1.4).
- **The offset has to be in the samples.** Since nothing carries a region's
  place, the studio would write each stem so its sample 0 is the MIDI's
  tick 0:
  - a positive `% Audio offset:` becomes that much silence before it;
  - a negative one becomes that much cut from its head;
  - the lead-in of §3.3 adds its own silence.

  Then *drop everything at bar 1* is the whole instruction, for the stems
  and the `% Audio:` recording alike. ffmpeg, which the studio already runs
  for Export Video, does this (`adelay`, `atrim`).
- **Sample rate.** The stems are 44.1 kHz. `video.ts`'s bounce of the parts
  is 48 kHz (§1.7). What GarageBand does with a 48 kHz file is for step 4.
  Bouncing at 44.1 kHz avoids the question.
- **Not known, for step 4:** whether several files dragged at once make a
  track each, all at the drop's position.
- **Lost:** volumes and mutes. Nothing else is lost, since audio is audio.
  The files are copied into the project, which is the default (§2.2).

### 3.5 MIDI and stems together

The two routes make one project:

1. Open the MIDI file. That gives a new project with the tempo map, the meter
   and a track per part.
2. Drag the folder's stems to bar 1.

Order matters. MIDI dragged into a project that already holds audio asks
whether to import its tempo. Yes leaves the audio out of sync with the new
grid, and No loses the tempo map (§3.3). The studio would write one
folder: `<score>.mid` and a WAV per stem or recording, all starting at tick
0, with a short *read me* of these two steps. It loses the mix, the mutes,
the track order and the colours, and anything §3.3 finds the MIDI loses.
§4 tests this route by hand.

### 3.6 AppleScript, Automator and Shortcuts

- **The dictionary** **[measured]**, from `sdef`, holds only:
  - the Standard Suite: `open`, `save` (with `as` and `in`), `close`,
    `count`, `make`, `quit`;
  - `document`, with `name`, `path` and `modified`, and `window`;
  - a Text Suite and print settings.

  It has no tracks, regions, tempo or import. `NSAppleScriptEnabled` is on,
  with no `OSAScriptingDefinition`: it is Cocoa's default dictionary. With
  the project chooser open, GarageBand answers `10.4.14, 0, Choose a
  Project` to `{version, count of documents, name of every window}`.
- **UI scripting** (System Events clicking menus and dragging) needs the
  Accessibility permission, which was denied here (§2.1). It breaks with any
  change to GarageBand's windows. It is not a route for a shipped app.
- **Shortcuts.** GarageBand has no App Intents metadata, no `NSServices`
  and no intents **[measured]**. Its only extensions are Quick Look,
  thumbnails and MIDI device plug-ins. Shortcuts can only *open a file in an
  app*.
- **What scripting could still do:**
  - **Launch GarageBand on the MIDI file.** `open -a GarageBand score.mid` is
    step 1 of §3.5. It goes through Launch Services and needs no
    permission.
  - **Have GarageBand write the `.band`.** `save document 1 in POSIX file
    "….band"` after `open` would make GarageBand write the project itself,
    with no writer of ours. It works (§4.5): a document opened this way is
    scriptable, and `save in` writes a whole `.band`. The stems still need
    placing by hand.
- **Permissions.** Lily Studio is signed with Developer ID and the hardened
  runtime, and is not sandboxed (`tauri.conf.json`). Sending Apple Events
  would need the `com.apple.security.automation.apple-events` entitlement,
  an `NSAppleEventsUsageDescription`, and the user's consent at the first
  use. `open -a` needs none of them.

### 3.7 A Logic Pro `.logicx` project

- **GarageBand cannot open one.** It declares the type
  `com.apple.logicx.project` but has no document type for it **[measured]**.
  Apple's Logic guide says *you cannot open Logic Pro projects in
  GarageBand*, nor export one that GarageBand reads, except as audio. Logic
  can share a project to GarageBand *for iOS*, not to the Mac.
- **It is a route into Logic.** Logic opens `.band` and `.logicx`, and a
  `.logicx` holds the same `ProjectData` (§2.5). LogicProFormatWriter (MIT,
  §2.7) writes one from MIDI and audio, and it places audio regions at
  positions, so the offset would survive. The cost and risk are those of a
  `.band` writer (§2.6), for Logic's owners only. There is no Logic here to
  test it. A `.band` writer and a `.logicx` writer would share most of
  their code. §8 plans no `.logicx` writer, since GarageBand refuses
  `ProjectData` that Logic wrote (§12).

### 3.8 MusicXML

- **GarageBand registers no MusicXML type**, and Apple's import guide does
  not mention it. Its strings do hold Logic's MusicXML importer
  **[measured]**: *Opening MusicXML File…*, *Parsing MusicXML File…*,
  *MusicXML Validation Error*, and the compressed `.mxl` errors. The parser
  ships in the shared engine, but no menu or document type is known to
  reach it. Whether a `.musicxml` dropped on the tracks area does is a
  one-minute check in step 4.
- **Even then it gains nothing over MIDI.** MusicXML carries notes, part
  names, MIDI programs and tempo, which the MIDI route carries too, and no
  audio. Its gain would be notation, and GarageBand's score view is only a
  view of the piano roll.
- **Neither LilyPond nor the studio writes MusicXML.** LilyPond 2.26 only
  reads it, through `musicxml2ly`. The route would need a writer of its own.

### 3.9 GarageBand for iOS

- **Import, per Apple's guide.** Files come in through the Loop Browser's
  *Files* tab, from the Files app or iCloud Drive.
  - **Audio:** AIFF, WAV, Apple Loops, AAC and MP3, so no CAF and no Apple
    Lossless. Only Apple Loops follow tempo.
  - **MIDI:** *GarageBand creates a new Keyboard track for each track in the
    MIDI file*, up to 32 tracks in all. A drum part arrives as a Keyboard
    track, not a Drums track.
  - **Sections:** the whole file comes in only when the song section is
    *Automatic*. Otherwise it is cut to the section.
  - **Not documented:** the tempo map, the programs, the names. Nothing was
    tested, since there is no iPad here.
- **From the Mac.** GarageBand on the Mac has *Share › Project to GarageBand
  for iOS* (to iCloud Drive) and *Import from GarageBand for iOS*
  **[measured]**, from its strings. A shared project *cannot have notes or
  other data before position 1 1 1 1*. The lead-in of §3.3 keeps
  everything after it. A Mac user could do §3.5, then share.
- **What the studio could give it.** The same folder as §3.5. On an iPad
  the web build (D55) can download the MIDI, but it has no stems (§1.1). A
  native iOS project is the top-level `projectData` archive (§2.4), so
  writing one is `.band` work again.

### 3.10 What step 4 should test

The MIDI-and-stems route of §3.5, on a score with:

- several staves, one without `midiInstrument`;
- a drum staff;
- two strings;
- `\key`, `\partial`, words;
- a second `\tempo`;
- `% BPM:` 113.76 and a non-zero `% Audio offset:`.

Record:

- **MIDI:** tracks and their header names; patches, including the unnamed
  part and the drums; key; tempo hundredths; lyrics; CC 7 and CC 10.
- **The lead-in:** whether the score's bar 1 falls on bar 2.
- **Stems:** what several WAVs dropped at once do; whether they follow the
  tempo; a 48 kHz WAV; a FLAC.
- **Order:** the tempo prompt when MIDI follows audio.
- **Scripting:** whether `open` then `save in` writes a `.band`.
- **MusicXML:** whether a dropped `.musicxml` is read.

## 4. MIDI and stems tested in GarageBand

### 4.1 The spike

`spike/make_import_bundle.py` makes the folder of §3.5 from a sample score.
It uses only the standard library, `lilypond` and macOS's `afconvert`:

```sh
python3 -I docs/research/garageband/spike/make_import_bundle.py OUT_DIR
```

- **The score** (`OUT_DIR/work/score.ly`) has what §3.10 asks for:
  - a Voice staff (`choir aahs`) with 18 syllables of words;
  - Violin and Cello;
  - a PianoStaff of two staves without `midiInstrument`;
  - a DrumStaff;
  - `\key g \major`, 3/4, `\partial 4`, ♩ = 112 and then ♩ = 84 at bar 5;
  - `% BPM: 113.76` and `% Audio offset: -2.40`.
- **The MIDI file** is lilypond's, rewritten as §3.3 proposes:
  - each track is named by its part, as `partNames` would name it (*Piano
    · G clef*);
  - each track gets a program change, CC 7 and CC 10. The Violin is panned
    to 40 and the Cello to 88, so their arrival can be seen;
  - the tempos are scaled to `% BPM:`: 112 becomes 113.76 and 84 becomes 85.32;
  - a lead-in of two quarters (a 3/4 bar less the pickup) moves every note,
    syllable and later tempo. Names, programs, controllers, the meter and
    the key stay at tick 0.
- **The audio.** There is no real recording, so the spike synthesizes the
  `% Audio:` file (`Song (recorded).wav`) from the rewritten tempo map. It
  places the file by D155's rule: the recording's time is the music's time
  less the offset. It has a click on every beat, higher on the downbeat,
  and a bass note on every downbeat. The two halves are the stems
  `Drums.wav` and `Bass.wav`.
- **The alignment.** The stems and `Song.wav` are cut or padded so their
  sample 0 is the MIDI's tick 0. The shift is the lead-in (1.0549 s) plus
  the offset (−2.40 s), so 1.3451 s is cut from the head. Before GarageBand,
  every click onset in `Drums.wav` was within 0.04 ms of a beat of the MIDI
  file, at 44.1 kHz and at 48 kHz **[measured]**.
- **`probes/`** holds the rest of §3.10:
  - `lilypond.mid`, the MIDI as lilypond wrote it;
  - `Drums 48k.wav` and `Drums.flac`;
  - `Probe.musicxml`, one bar for a flute;
  - `Lean.mid`, the rewritten file without the words' track and without
    the control track's name and text events (§4.3).

### 4.2 How it was run

On GarageBand 10.4.14, on 2026-11-03, with the user at the screen:

- **Opened from the terminal.** `open -a GarageBand "Lily Import Test.mid"`
  made a new project, *Untitled 2* (path
  `~/Music/GarageBand/Untitled 2.band`, not yet written).
- **Read back through AppleScript.** `save document 1 in POSIX file
  "…/x.band"` saved each state to a scratch folder, where
  `inspect_band.py` read it (§4.5).
- **Watched by the user.** Screen capture is denied to the terminal, so the
  user dragged the files, played the project, and sent screenshots of the
  tracks.
- **One project at a time.** GarageBand holds one project. Opening a second
  file while one is open asks whether to save or replace the first.

**[measured]** below marks both what the saved `.band` showed and what the
user saw and heard. Each claim says which.

### 4.3 What the MIDI file brought

| What | Arrived | How it shows |
| --- | --- | --- |
| **A track per MIDI track** | yes, **including the control track and the words' track** | 8 tracks for 6 parts. The control track became a track whose region is named after the title (*Lily Import Test*), with its text events drawn as `#`. The words' track became a track named *Words*, its syllables drawn as `#`. Both got *Steinway Grand Piano* and play nothing. `Lean.mid`, without them, gave exactly the 6 parts |
| **Part names** | as the **regions'** names, UTF-8 intact (*Piano · G clef*) | The track headers show the patch, as *String Ensemble \| Ch1*, on every track (screenshots). The name is in each `MSeq` of the saved file |
| **Programs** | yes, through the fixed table of §3.3 | 52 → *Classical Ensemble*, 40 and 42 → *String Ensemble* (both), 0 → *Steinway Grand Piano*. The region shows the program in a box (`52`, `40`, `00`) |
| **A track without a program change** | *Steinway Grand Piano*, not *Classic Electric Piano* | The title and words tracks have none (§3.3's worry about the default patch does not apply) |
| **Drums** (channel 10, program 0) | the *SoCal* kit, as a software-instrument track with a MIDI region | Not a Drummer track. The header says *Ch1* like every other track, and the region's events keep channel 10 |
| **Tempo map** | yes, **to the hundredth** | 113.76 at bar 1 and 85.32 at bar 6 (quarter 15). The `Song` record stores bpm × 10 000. `MetaData.plist` has 113.76 as a float |
| **Meter and key** | yes | 3/4 and G major in `MetaData.plist`. The key came from the staves' tracks (§1.2) |
| **The lead-in** | yes | The pickup sounds on beat 3 of bar 1. **The score's bar 1 is GarageBand's bar 2**. Notes are rescaled from 384 to 960 a quarter |
| **Volume and pan** (CC 7, CC 10) | as **events in the regions**, not as the tracks' settings | After playing, the Violin's pan knob was turned left, the Cello's right, and the faders differed. In a screenshot of a newly opened copy, every strip was at its default. So they seem to act like automation: the mix is right once the project plays from the start. Whether they then override a fader moved by hand was not tried |
| **Words** | kept, unusable | Lyric events are text records (`0x70`, *la*) in the *Words* region. GarageBand shows no lyrics |
| **The title** | not as the project's name | The project stays *Untitled N*. The title only names the control track's region |
| **Track order** | yes | The MIDI file's order |

### 4.4 What the stems brought

- **Several WAVs dropped at once** at bar 1 made **a track each, all at bar 1**
  **[measured]**, from the screenshot and the file. Each region's place is
  34 560, the value step 2 found for bar 1 of a 4/4 project (§2.1). In this
  3/4 project it is still bar 1, so that value is a fixed origin, not *one
  4/4 bar early*. Each track is named after its file (*Song*, *Drums*,
  *Bass*). GarageBand chose the icons by name: a bass guitar for *Bass*, a
  drum kit for *Drums*.
- **In time throughout.** Played from bar 1, the clicks of `Drums.wav`
  sounded together with the SoCal kit to the end, across the tempo change
  **[measured]**, by ear. In the screenshot, every bass note of `Bass.wav`
  starts with the MIDI notes of its bar. The audio is placed in time and not
  stretched. That is right here, since the MIDI's tempo map is the
  recording's.
- **Copied, unchanged.** The WAVs land in `Media/Audio Files/`. Each copy's
  `data` chunk is byte for byte the spike's. GarageBand appends its own
  `LGWV` chunk of 5 986 bytes, the waveform overview §2.8 asked about
  **[measured]**.
- **48 kHz** is accepted, at bar 1. The project's copy is **converted to
  44.1 kHz**, with the same length **[measured]**.
- **FLAC is refused** when dragged, and so is **MusicXML** **[measured]**,
  by the user.
- **MIDI after audio.** A MIDI file dragged into the project after the stems
  asked whether to import its tempo. *No* kept the project's 113.76 and
  85.32 **[measured]**. The file dragged was step 2's one-track
  `lily-test.mid`, not `probes/lilypond.mid`. The prompt does not depend on
  the file, and the raw file's patches are covered by the rows above.

### 4.5 AppleScript can have GarageBand write the `.band`

**[measured]**: after `open -a`, the new document is scriptable. Its `name`,
`path` and `modified` can be read, and `save document 1 in POSIX file
"…/x.band"` writes a complete project with no window touched. The saved
package holds the MIDI tracks as §4.3 lists them and, after the stems were
dropped, the WAVs in `Media`. `close document 1 saving no` closes it. Three
limits remain:

- **The stems.** Nothing in the dictionary places audio, so the drop at bar 1
  stays a hand step.
- **Permission.** It needs the Apple Events consent of §3.6, here given to
  the terminal in step 3. The studio would need its own entitlement and
  consent.
- **The open project.** `open -a` with a project already open stops at the
  save-or-replace question (§4.2), which a script cannot answer.

So *open the MIDI in GarageBand and save it as `<score>.band` beside the
score* can be done by the studio, with GarageBand as the `.band` writer, for
the MIDI half of the export.

### 4.6 What it means for an export

- **The route works.** Open the MIDI file, then drop every WAV at once at
  bar 1. That gives a project whose tempo map, meter, key, parts and audio
  line up, with the score's bar 1 at bar 2 after a pickup.
- **The rewriter of §3.3, amended:**
  - the track names, programs, tempos at `% BPM:` and the lead-in work as
    planned;
  - **drop the words' tracks and the control track's name and text
    events** (`Lean.mid`). Otherwise each is an empty piano track;
  - **CC 7 and CC 10 are optional.** They give the studio's mix only as
    playback events, not as the tracks' settings, and may fight the
    user's faders (untried). §6.3 leaves them out.
- **The stems.** WAV, padded or cut to tick 0 (§3.4), as the spike does with
  `align`. 48 kHz is fine. A FLAC from karaoke-mlx must be converted.
- **Names.** Only the regions carry the part names, and a track header shows
  its patch. Two strings both read *String Ensemble*. The WAVs' file names
  are the audio tracks' names, so the stems should be named by part
  (`Vocals.wav`, not `<song> - vocals.wav`).
- **Lost:** the track headers' names, the mutes (a muted part is a missing
  track), the volumes as settings, the words, and the title as the project's
  name.
- **§1.9's questions.** A pickup does shift the bars, and the lead-in puts the
  score's bar 1 on bar 2. A negative `% Audio offset:` survives as a cut
  made in the file, not as a trimmed region. The region sits at bar 1 either
  way.
- **For step 5.** The region origin 34 560 is bar 1 in 3/4 as in 4/4.
  GarageBand rewrites each audio file with an `LGWV` chunk of its own. A
  `.band` that GarageBand writes through `save in` is a template to compare
  a patched one against.

## 5. Patching a `.band` that GarageBand saved

### 5.1 The spike

`spike/patch_band_template.py` copies a `.band` that GarageBand saved, once
per variant, and changes only the parts §2.6 found readable. Its second form
has GarageBand open each copy and save it again, and reads back what
GarageBand kept:

```sh
python3 -I docs/research/garageband/spike/patch_band_template.py TEMPLATE.band OUT_DIR
python3 -I docs/research/garageband/spike/patch_band_template.py --open OUT_DIR
```

- **The template** is step 4's `stems.band`: `Lily Import Test.mid` opened,
  the three WAVs dropped at bar 1, and the project written by `save in`
  (§4.5). It has 8 MIDI tracks and 3 audio tracks, and tempos of 113.76 and
  85.32. Its variant names are written into the script.
- **How a patch is written.** The spike splits `ProjectData` into its
  records, changes record bodies in place or resizes them, and joins them
  again. It rewrites every record's length and the file header's length. No
  record is added or removed. `MetaData.plist` is changed to agree, and so is
  `Media/Audio Files/` when an audio file is swapped. Every other file is
  copied unchanged, including `WindowImage.jpg`, the `DisplayState` files,
  `ProjectInformation.plist` and the top-level `projectData`.
- **How it was checked** **[measured]**, on 2026-11-03, with GarageBand
  10.4.14:
  - **AppleScript.** `open` each copy, then read `name of every window` and
    `modified`. Then `save document 1 in` a new path and `close saving no`.
  - **Reading back.** `inspect_band.py` and the spike read the project
    GarageBand saved: its tempos, its region and strip names, its `AuFl`
    records, and the samples in its `Media/`.
  - **By eye.** The user looked at `all.band` and played it.

  Every patched variant was run twice from a fresh output folder, with the
  same results.

### 5.2 What was patched

| Variant | Records changed | Size change |
| --- | --- | --- |
| `tempo` | each tempo event in EvSq class 3 scaled so the first is 100 (113.76 → 100, 85.32 → 75); `Song` +110 and +114; `MetaData`'s `BeatsPerMinute` | none |
| `tempo-meta` | `MetaData`'s `BeatsPerMinute` only | none |
| `rename-same` | the region `MSeq` *Violin* → *Fiddle* | none |
| `rename` | the region `MSeq`s *Violin* → *Violin I (solo)*, *Cello* → *Violoncello*, *Drums* → *Drum kit*; channel strips `Envi` 44 and 45 *String Ensemble* → *Violin I (solo)* and *Violoncello* | records grow and shrink |
| `media-same` | `Song.wav`'s samples swapped for a tone of the same length, with no `LGWV`; `AuFl`'s size and `LGWV` size | none in `ProjectData` |
| `media` | `Song.wav` replaced by `Voice take 2.wav`, 9 s: `AuFl`'s name, size, frames and format; `AuRg`'s name and length; the audio strip's `Envi` name; `MetaData`'s `AudioFiles` | records grow |
| `all` | `tempo`, `rename` and `media` together | |
| `stale-path` | `media-same`, and `AuFl`'s folder set to `/Users/Shared/No Such Project.band/…`, which does not exist | none |
| `other-path` | `media-same`, and `AuFl`'s folder set to the absolute path of the *template's* `Media/Audio Files`, which still holds the old `Song.wav` | none |

**`AuFl`, the audio file record,** now has a known layout. The body starts
with two u32s (`0x390`), then a u16 count of characters and the name in
UTF-16LE. Every field after the name is at a fixed offset from the name's
end, so the record is 588 bytes plus twice the name's length:

| From the name's end | Field |
| --- | --- |
| +0x000 | `LFUA` |
| +0x08A | the folder, 256 bytes, NUL-padded: absolute (`/Users/…/audio.band/Media/Audio Files`, step 2's project, saved with File › Save) or relative (`Audio Files`, the projects written by `save in`) |
| +0x196 | u32, the file's size in bytes |
| +0x1C8 | `EVAW`, u32 0, u32 offset of the samples (44), u32 frames, u32 0, u32 rate, u16 channels, u16 bits |
| +0x212 | u32, the size of the `LGWV` chunk at the end of the file (1388 and 5986 in step 2 and step 4) |

`AuRg` holds the region's length in frames at +0x16 and its name at +0x4A.
A channel strip's name is at `Envi` +0x9E. Both names are a u16 length
followed by UTF-8, as in `MSeq` +0x10.

### 5.3 What GarageBand did with it

| Variant | Opens | Marked modified on open | Kept after `save in` |
| --- | --- | --- | --- |
| `copy` | yes, no dialog | no | everything. Only the per-record counter (MSeq +8) changed |
| `tempo` | yes, no dialog | no | **100 and 75**, and `MetaData` 100 |
| `tempo-meta` | yes, no dialog | no | 113.76 and 85.32: **`MetaData.plist` is ignored on open** and written again from `ProjectData` |
| `rename-same` | yes, no dialog | no | *Fiddle* |
| `rename` | yes, no dialog | no | the three region names and the two strip names |
| `media-same` | yes, no dialog | **yes** | the swapped samples. An `LGWV` of 5 986 bytes was appended to the WAV |
| `media` | yes, no dialog | **yes** | *Voice take 2.wav*, 396 900 frames, with the new samples, an `LGWV` of 3 110 bytes, and the audio track named *Voice take 2* |
| `all` | yes, no dialog | yes | all of the above together |
| `stale-path` | yes, no dialog | yes | the swapped samples from the package's own `Media/` |
| `other-path` | yes, no dialog | yes | **the package's samples, not the template's**, although the folder named in `AuFl` exists and holds a `Song.wav` |

**By eye** **[measured]**, the user opened `all.band` and played it from
bar 1, and there was no dialog. The LCD showed 100. The fourth and fifth
track headers read *Violin I (solo)* and *Violoncello*, not *String
Ensemble*. The regions had the new names. The last audio track was *Voice
take 2*, a 9-second region of beeps.

What this answers from §2.8:

- **Which file wins.** `ProjectData`. `MetaData.plist` is a summary that
  GarageBand writes on save and does not read when it opens a project.
- **Moved media.** GarageBand finds an audio file by its name in the
  package's `Media/Audio Files/`. The folder in `AuFl` does not matter when
  the file is there, whether that folder is missing or another real one.
  Re-saved, the folder becomes the relative `Audio Files`.
- **The `LGWV` chunk is not needed.** GarageBand writes one into the WAV
  inside the opened package and marks the project modified, so closing it
  asks whether to save. The samples are unchanged.
- **Names of another length** work in regions and channel strips, as long
  as the record's length grows with them and the fields after the name keep
  their even offset. `rename` broke that rule for *Violin I (solo)*, and
  GarageBand saved that region's fields a byte out of place without a sign
  (§11.4). A channel strip's name is what
  the track header shows (§4.3 found the header showing the patch). The
  rename touches only the name. The strip's plug-in records stay as they
  were, and whether its patch changed was not checked by ear.
- **The project's name.** The window takes the package's file name
  (`rename.band - Tracks`). `ProjectInformation.plist` still says *stems*,
  and nothing needed changing there.

### 5.4 What a mistake does

Three controls, each a bug a patcher could have, were opened in the same way
**[measured]**:

| Control | What is wrong | What GarageBand did |
| --- | --- | --- |
| `bad-length` | the file header's length is 16 bytes too long | opened it with no dialog and did not mark it modified, then saved a correct file. **The header's length is not checked** |
| `bad-record` | the region `MSeq` *Violin*'s length is one byte too long, so every record after it is read from the wrong place | **GarageBand crashed** while opening it, with `EXC_BAD_ACCESS` (SIGSEGV) at address 0. There was no error dialog, and the crash report is in `~/Library/Logs/DiagnosticReports/`. When AppleScript relaunched it, GarageBand reopened the last project opened before it (`other-path`), not the one that crashed it |
| `bad-name` | *Violin*'s name length is 10 for 6 bytes, inside a record whose own length is right | opened it with no dialog and saved it. The name came back as *Violin*, since the 4 extra bytes were NULs, but **36 bytes after the name were changed**: GarageBand read every field after the name 4 bytes late, and saved what it read |

So GarageBand checks nothing that the spike could find. It crashes on a
record that is framed wrong, and it accepts a field that is wrong and saves
it. Nothing tells the user which of their projects is damaged. A patched
file that GarageBand opens with no dialog is therefore not proven sound, and
§5.3's results rest on reading back what GarageBand saved.

### 5.5 Is patching `ProjectData` workable?

**For changing values in a project of a fixed shape: yes.** Tempo changes,
names of any length and swapped audio files of any length all opened with no
dialog. All of them survived GarageBand's own save, and they looked right on
screen. A patcher needs only what §5.2 lists:

- the record lengths;
- the strings' own lengths;
- `AuFl` and `AuRg` for an audio file.

It does not need `MetaData.plist` (though writing it keeps Finder's summary
right), the header's length, an `LGWV` chunk, or a correct folder in `AuFl`.

**As a general `.band` writer: not by patching alone.** What §5 tested
cannot change the shape of a project. Adding or removing a track, a region,
a note or an audio file means creating records, and when §5 was run their
links to one another were opaque. A template's tracks are fixed, so a
template with 8 tracks cannot hold a score with 9 parts. Nor was changing
an instrument tried: it lives in plug-in state (`AuCO`/`AuCU`).

§11 has since decoded the links, and §13 adds tracks, regions and audio
files to a project GarageBand saved: 14 tracks from a base of 2, which
GarageBand opens, plays and saves whole **[measured]**. So the shape is no
longer fixed by the template. The instrument still is: every new track
copies its plug-in state from the base (§13.5).

**The risks:**

- **No safety net.** One wrong length crashes GarageBand, and one wrong
  field corrupts the project silently (§5.4). A writer would need its own
  check that walks every record, and a test against each GarageBand release.
- **Format churn.** The template's format (0x09d0, header form 2) is
  whatever the GarageBand that saved it wrote (§2.7). A patcher must refuse
  a format it was not tested on.
- **A template per shape.** The template must have been saved by GarageBand.
  It carries its own patches, screenshot and window state.

**A smaller use that works.** §4.5 has GarageBand write the `.band` from the
MIDI file. The track headers then show the patch, so two strings both read
*String Ensemble* (§4.6). Renaming the channel strips of that saved project
is §5's `rename`, which works. It would give the headers the part names that
the MIDI route loses, and it changes no project's shape. When §5 was run,
which strip belongs to which track was not known. Here the strips' indexes
(41 to 48) follow the MIDI file's track order. §11.2 has since found the
link: a track row's body +0x06 names its strip's `Envi`. It is the first
part of §8's stage 5.

## 6. Recommendation

### 6.1 In short

**MIDI and stems, in a folder that the user opens in GarageBand.
GarageBand writes the `.band`. Later, Lily Studio may edit the `.band` that
GarageBand saved, but it never writes one from nothing.**

- **The export** is a folder, `<score> (GarageBand)/`, holding:
  - `<score>.mid`, lilypond's MIDI rewritten for GarageBand (§3.3, as §4.6
    amends it);
  - a WAV for the recording and for each of the song's files that the score
    names, each starting at the MIDI's tick 0 (§3.4);
  - `Read me.txt`, with the two steps of §3.5 and what is lost (§6.2).
- **On the Mac**, the studio then opens the MIDI file in GarageBand. That
  makes the project, with a track per part, the tempo map, the meter and the
  key (§4.3). The user drops every WAV at once at bar 1, which places them in
  time (§4.4), and saves the project.
- **On the web** the export is the rewritten MIDI file alone, as a
  download. The web has no audio (§1.1), and a browser cannot open
  GarageBand.
- **No `.band` from nothing**, and no project of Apple's shipped in the app.
  A later stage (§8, stage 5) edits the `.band` that GarageBand saved in
  stage 4. It names the track headers, then adds the stems as audio tracks,
  so the user no longer drops them by hand.

The options weighed:

| Option | What the user gets | What it costs | Verdict |
| --- | --- | --- | --- |
| **A `.band` the studio writes whole** | the whole project, with the stems placed and the headers named | a base that GarageBand saved, shipped in the MIT app (§7.2), since GarageBand opens only its own saves (§12); every new MIDI track with the base's instrument, since the plug-in state is copied (§13.5), where the MIDI import picks from 72 patches; refused at any format but the base's; a crash or a silent corruption for any mistake (§5.4); a test against every GarageBand release | **no** (§8, not planned) |
| **MIDI and stems** | everything of §4.6: parts, names on the regions, instruments, tempo map to the hundredth, meter, key, bar 1, the audio in time. One drag by hand | a MIDI rewriter, a WAV aligner and a folder, in standard formats only | **yes** (§8, stages 1 to 3) |
| **Both: MIDI and stems, then a `.band` that GarageBand writes** (§4.5) | the same project, saved by GarageBand as `<score>.band` beside the score, so ⌘S saves it there and not to `~/Music/GarageBand/Untitled N.band` | the Apple Events entitlement, a consent dialog, and the open-project limit (§4.5). The stems are still dropped by hand | **later, if asked for** (§8, stage 4) |
| **Both, then the studio edits that `.band`** | the part names on the track headers (§5.5), and the stems placed with no drag (§13) | a Rust port of the spikes' record walker, link check and track copier; a gate on the tested format; a check by hand on each GarageBand release. Not tried: an audio track added to the MIDI import's save, which has none to copy | **later, after a spike** (§8, stage 5) |

**What §9 to §13 changed.** D161 ruled out a writer because the links
between records were opaque. They are not. §11 decoded them on 31
projects, and §13 built a 14-track project from a 2-track one that
GarageBand opened, played and saved whole **[measured]**. A writer is now
a matter of engineering, not of guessing. Four findings still keep it
from replacing the MIDI route:

- **Only GarageBand's own saves open** (§12). LPFW's projects and Logic's
  are refused, so a writer starts from a `.band` GarageBand saved. Shipped
  in the app, that base is Apple's bytes (§7.2). The user's own save is not.
- **The instrument comes with the strip** (§13.5). Until the plug-in state
  is decoded, a new MIDI track sounds like the track it was copied from.
  The MIDI import already gives each part its own patch.
- **GarageBand checks nothing** (§5.4). A mistake crashes it or corrupts
  the project silently, so a writer needs its own check of every link.
- **One base and one version.** §13 used one project saved by 10.4.14. What
  GarageBand needs of the bookkeeping that §13 wrote was not tried (§11.6).

So a writer earns its place only where the MIDI route falls short: the
track headers' names and the drag of the stems. It edits the user's own
save, which ships nothing of Apple's and is in the format of the user's
GarageBand.

### 6.2 What reaches GarageBand, and what does not

- **Arrives** (§4.3, §4.4) **[measured]**:
  - a track per part, in the score's order;
  - the part names, as the regions' names;
  - the General MIDI programs, through GarageBand's table;
  - the tempo map at `% BPM:`, to the hundredth;
  - the meter and the key;
  - bar 1: the score's bar 1 is GarageBand's bar 1, or bar 2 after a pickup;
  - the recording and the stems, in time across tempo changes.
- **Lost**, and said in `Read me.txt`:
  - the track headers' names, which show the patch, so two strings both
    read *String Ensemble* (until stage 5 names them);
  - the mix: the volumes, and the mutes (a muted part is left out, as in
    D54);
  - the words and the title;
  - the instruments beyond the 72 patches of GarageBand's table (§3.3).

### 6.3 Choices inside the route

- **A new export, beside Export MIDI.** D54's file keeps lilypond's bytes,
  and other programs read it. The lead-in and the dropped tracks of §4.6
  are for GarageBand. The rewriter's other parts (names, the fold's
  programs, `% BPM:`) would also do D54's and D155's *Not done*. Whether
  Export MIDI takes them is a decision of its own, not part of this one.
- **The rewrite** is that of `spike/make_import_bundle.py` (§4.1), less the
  controllers:
  - a track-name event from `partNames`;
  - a program change on every track, from the Parts fold's choice or the
    written instrument;
  - the tempo events scaled by `atTempo`'s ratio for `% BPM:`;
  - a lead-in of a bar less the pickup, from `barOneTime` and the first
    meter, when there is a pickup;
  - no words' tracks, and no name or text events in the control track
    (`Lean.mid`);
  - the muted parts' tracks left out, behind D54's checkbox.
- **No CC 7 or CC 10.** They arrive as events in the regions, not as the
  tracks' settings, and may override the user's faders (§4.3, untried).
  The mix is lost instead, and the read-me says so. A later test can bring
  them back.
- **Which audio.** The files on the score's timeline:
  - the `% Audio:` recording, unless the fold mutes it, shifted by its
    offset (D155);
  - `% Backing:`, `% Music:` and `% Stem:`, which D133 and D135 play on the
    score's own time.

  Every file also takes the lead-in. Each is named for what it is:
  `Vocals.wav` from `<song> - vocals.wav`, and the recording by its own
  name. Whether a backing track stays in time when `% BPM:` differs from
  the score's tempo is checked by ear in stage 2.
- **Aligned by the studio.** A WAV is padded with silence or cut at its
  head in Rust, at its own rate and depth. GarageBand takes 48 kHz (§4.4).
  FLAC and MP3 go through ffmpeg when it is installed (`find_ffmpeg`, as
  Export Video uses). Without ffmpeg, the export leaves them out and says
  which. GarageBand refuses FLAC (§4.4), and MP3 adds its encoder's delay
  (§1.5).

## 7. Risks

### 7.1 Format churn

- **The recommended route uses no Apple format.** MIDI and WAV are
  standard, and GarageBand writes its own `.band` in whatever format its
  version uses. The `.band` format changed between 10.2.0 and 10.4.14 (format
  code, header form, record versions: §2.7), and GarageBand checks nothing
  (§5.4). Stages 1 to 4 do not carry that risk. Stage 5 does, so it
  refuses any format it was not tested on, and each new GarageBand version
  is tested before its format is let in (§8).
- **What can still change is GarageBand's importer:** its General MIDI table
  (`GM_Instrument_Mapping.plist`), the empty tracks it makes of a track
  without notes, the drum kit it picks, the tempo prompt (§4.3, §4.4). Each
  would change what arrives, not break the project. `make_import_bundle.py`
  repeats §4 on a new major version of GarageBand, and the read-me names the
  version it was tested with.
- **Stage 4's scripting** rests on Cocoa's default dictionary
  (`NSAppleScriptEnabled`, §3.6), which Apple could drop. Stage 3's `open`
  does not.

### 7.2 Licensing

- **Nothing of Apple's is shipped.** The export holds lilypond's MIDI and
  the user's own recordings and stems. GarageBand's sounds play on the
  user's Mac, under GarageBand's licence.
- **A template would ship Apple's bytes.** A `.band` saved by GarageBand holds
  the iOS stub that is byte for byte the one in GarageBand's own templates
  (§2.4), the state of Apple's plug-ins and patches (§2.5), and a
  screenshot. Shipping one inside the MIT app is redistributing Apple's
  data. That is why stage 5 edits the `.band` the user's own GarageBand
  saved, and the app ships no base.
- **Reverse engineering.** §2, §5 and §10 to §13 read files that
  GarageBand saved, and never GarageBand's code. Apple's software licences
  forbid decompiling except as the law allows. Stages 1 to 4 need no format
  knowledge at all; stage 5 needs what those sections decoded. That is not
  legal advice, and no lawyer has looked at it.
- **Other people's code.** Stages 1 to 4 need none. Stage 5 ports the
  spikes here, which are the repository's own, under MIT. MIT and
  Apache-2.0 projects could come in with their notice; the GPL projects
  may only be read, and projects with no licence not used at all (§9.5).
- **The name.** *Export for GarageBand…* names Apple's trademark to say what
  the export is for, and uses no Apple icon or artwork.

### 7.3 Sandboxing and permissions

- **Lily Studio** is signed with Developer ID and the hardened runtime, and
  is not sandboxed (`tauri.conf.json`). Writing a folder beside the score,
  running ffmpeg and opening a file in another app need nothing new.
  Opening goes through Launch Services, which needs no entitlement.
- **Stage 4's Apple Events** need the
  `com.apple.security.automation.apple-events` entitlement and an
  `NSAppleEventsUsageDescription`. macOS also asks the user once, under
  *Privacy & Security › Automation*, and the user can later revoke it. A
  refusal (`-1743`) must leave stage 3's result: the project open and
  unsaved. If the studio were ever sandboxed, for the Mac App Store, it
  would also need `temporary-exception.apple-events` for
  `com.apple.garageband10`, which review may refuse. Stages 1 to 3 would
  survive sandboxing, with the folder chosen in a save panel.
- **GarageBand is sandboxed** (`com.apple.security.app-sandbox`) with a
  temporary exception that reads and writes every path (`//`)
  **[measured]**, from its signature. That exception is probably what let
  §4.5's `save in` write to a scratch folder. A GarageBand without it could
  still open a file passed by `open`, but stage 4's save beside the score
  might then fail.
- **The web** (D55) runs each user's compile in a sandboxed worker, which
  writes nothing for the user. The browser downloads one file. It would need
  a zip only if the web ever had audio.

## 8. A staged plan

Each stage is one reviewable change, with its own D-entry or one that
supersedes D162, and is checked by hand in GarageBand as §4.2 was. No test
can run GarageBand, so the tests check the files.

| Stage | What | Where | Mac app | Web |
| --- | --- | --- | --- | --- |
| **1** | The MIDI rewriter and *Export for GarageBand…* writing the `.mid` | `crates/session/src/midi_export.rs`; the renderer's plan from `playbackSetup.ts` and `tempo.ts`; both export commands | yes | yes |
| **2** | The folder: the aligned WAVs and `Read me.txt` | a WAV aligner in `crates/session`; the Tauri command | yes | no |
| **3** | Open the MIDI file in GarageBand after the export | `src-tauri` | yes | no |
| **4** | Optional: GarageBand saves `<score>.band` beside the score | `src-tauri`, the entitlement and `Info.plist` | yes | no |
| **5** | Later, after a spike: the studio edits that `.band`, naming the headers and adding the stems | a `band` module in `crates/session`; `src-tauri` | yes | no |

**Stage 1. The MIDI rewriter (the studio, both builds).**

- `midi_export.rs` gets a rewrite beside `without_tracks`. The renderer
  sends a plan with the bytes: the tracks to drop, each kept track's name
  and program, the tempo ratio, and the lead-in in ticks. The renderer
  already has each of them: `partsOf` and `partNames`, the fold's
  instruments, `atTempo`'s ratio, and `barOneTime`. The Rust side only
  edits events, so the Mac app's command and the web's worker share it, as
  they share `without_tracks`.
- **The menu.** *File › Export for GarageBand…*. On the Mac, a save panel
  writes `<score>.mid` until stage 2 makes it a folder. On the web, the
  file is downloaded as Export MIDI is (`src/web/transport.ts`).
- **Tests.**
  - Rust unit tests on lilypond's MIDI with a pickup, two tempos, words and
    an unnamed piano part, reading back each event that §4.1 rewrites;
  - a renderer test of the plan;
  - the smoke test writing the file;
  - by hand: the file opened in GarageBand, saved by AppleScript and read
    with `inspect_band.py`, against §4.3.

**Stage 2. The folder (the Mac app).**

- The Tauri command writes `<score> (GarageBand)/`, asking before it
  replaces one. It holds the `.mid`, the WAVs of §6.3 and `Read me.txt`.
- **The shift.** A file's sample 0 goes to the MIDI's tick 0. The shift, in
  seconds, is the lead-in at the rewritten first tempo, plus `% Audio
  offset:` for the recording. A positive shift pads silence, and a negative
  one cuts the head.
- **Tests.**
  - unit tests of the aligner: 16-bit, 24-bit and float WAV, padded and
    cut, and the frames checked;
  - a test that the export leaves out a FLAC without ffmpeg and says so;
  - by hand: §4's drop and listening, with a real recording and a
    negative offset.

**Stage 3. Open in GarageBand (the Mac app).**

- After the export, the studio opens the `.mid` with the app whose bundle
  id is `com.apple.garageband10`, through `NSWorkspace` or `open -b`.
- It does so only when Launch Services finds that app. Without it, the
  export just reveals the folder in Finder.
- GarageBand's own dialogs stay its own: the save-or-replace question when a
  project is open (§4.2), and the *Essential Sounds* download at a first
  launch (§2.1).

**Stage 4, optional. GarageBand saves the `.band`.**

- **Only if users ask for it.** Its gain is where ⌘S saves.
- After stage 3, the studio waits for the new document, then sends
  `save document 1 in` `<score> (GarageBand)/<score>.band`, as §4.5 did.
- It needs §7.3's entitlement, usage string and consent. Before sending, it
  refuses when `count of documents` is not 1. A refusal of permission leaves
  stage 3's result.

**Stage 5, later. The studio edits GarageBand's save (the Mac app).**

- **After stage 4, and after a spike.** It needs stage 4's save as its
  base, so it ships no base of its own (§7.2). It is started when users ask
  for the header names or miss the drag, and only once a spike, run as
  §13.1's was, answers what §13 left open for this base:
  - **An audio track in a save that has none.** The MIDI import's save
    holds MIDI tracks only, and §13 copied an audio track from the base's
    own. The spike tries one built from a MIDI track's rows and an audio
    strip, if the save has a spare one (§13.2). If GarageBand does not take
    it, stage 5 names the headers only, and the drag stays a hand step.
  - **The base it will meet:** stage 1's MIDI file of a score with a
    pickup, two tempos and eight parts, opened and saved by stage 4.
  - **What GarageBand needs** of the registry, the counters and the
    automation rows (§11.6). Writing all of them works (§13); a port that
    writes fewer has fewer ways to go wrong.
- **What it does**, in order of risk:
  1. names each track header for its part: it renames the `Envi` that the
     part's track row names (§11.2), as §5's `rename` did **[measured]**.
     No record is added;
  2. adds each WAV of stage 2 as an audio track with a region at the
     MIDI's tick 0, by §13's copy of a track **[measured]** on `c3-copy`,
     and the file into `Media/Audio Files/`. `Read me.txt` then loses its
     drag step.
- **The flow.** Stage 4 saves `<score>.band` and closes it (`close saving
  no`). The studio writes the edited project beside it, checks it, and
  only then replaces the save and `open`s it. A refused or failed edit
  leaves GarageBand's save as it was, and stage 4's result stands.
- **The gate.** It refuses a project whose format code and header form are
  not ones it was tested on (`0x09d0`, header form 2, from 10.4.14), whose
  links do not check before and after the edit, or that holds what the
  port does not know, such as a Drummer track (§11.6).
- **The code.** A `band` module in `crates/session`, ported from the
  repository's own MIT spikes:
  - the record split and join of `patch_band_template.py` (§5.1);
  - the link check of `inspect_band.py --links` (§11), and the record
    diff of `diff_band.py` (§10.2) for its tests;
  - the track copier and the registry of `build_band.py` (§13).

  From others' code, only what is MIT or Apache-2.0, with its notice
  (§9.5): LPFW's encodings of notes, regions and audio files (§9.6.6), and
  gb-in-Claude's of an audio file in GarageBand (§9.2). No GPL code, no
  code without a licence, and no bytes of Apple's: every record added is
  a copy of one in the user's own save.
- **Tests.**
  - Rust unit tests: split and join give back the same bytes; every edit
    passes the link check; a mis-framed record or an unknown format is
    refused.
  - A GarageBand save is Apple's bytes and is not committed. Tests that
    need one read it from a path in the environment and skip without it,
    as tests skip without lilypond. A skipped run has not verified the
    stage.
  - By hand, on each change and on each new GarageBand version before its
    format is let through the gate: §13.1's loop (`open`, `save document
    1 in`, `close saving no`), the save read back with `inspect_band.py
    --links`, no new report in `~/Library/Logs/DiagnosticReports/`, and
    §13.4's look and listen at the screen.

**Not planned**, and what would reopen each:

- **A `.band` written whole**, from a base the app ships. Reopened if a new
  track's instrument can be chosen, which needs the plug-in state of
  `AuCO` and `AuCU` decoded (§11.6), and a base can be had without
  shipping Apple's bytes. Until then the MIDI import chooses the sounds
  better, and stage 5 gives the rest.
- **A `.logicx` writer.** GarageBand cannot open a `.logicx` (§3.7) and
  refuses `ProjectData` that Logic or LPFW wrote (§12).
- **The mix in stage 5's `.band`**: the volume, pan and mute in the
  strip's `AuCO`. Reopened after stage 5, once those fields are read in
  GarageBand's files (§2.6), where they were not checked.
- **MusicXML and a native GarageBand for iOS project** (§3.8, §3.9). On an
  iPad, the web's `.mid` goes through Files, which is untested.

### 8.1 Stages 1 to 3 as built, checked in GarageBand

On 2026-11-04, on GarageBand 10.4.14 and GNU LilyPond 2.26.0, stages 1 to 3
as built (D162 to D165) were checked as §4.2 checked the spike. The files
and the save were read by the terminal. What needs eyes and ears is left to
the operator below.

**The export.**

- **The score** is §4.1's, saved as `Lily Import Test.ly`, beside the
  spike's `Song (recorded).wav`. Two stems are named as karaoke-mlx names
  them, on the score's time: `% Backing: Lily Import Test - bass.wav` and
  `% Stem: Lily Import Test - drums.flac`.
- **The studio's own code**, each half as the app runs it:
  - the score compiled by `lily-outcome`, as the app compiles it;
  - the plan made by `garageBandPlan` and `garageBandAudio`, as the player
    makes it;
  - the folder written by `for_garageband` and `write_folder`, as
    `export_garageband` writes it once its panel has answered, with ffmpeg
    from Homebrew.
- **Not driven:** the menu, the panel and the alert. UI scripting is denied
  to the terminal. The smoke test drives them, without GarageBand.

**The folder [measured].** It holds `Lily Import Test.mid`,
`Song (recorded).wav`, `Bass.wav`, `Drums.wav` and `Read me.txt`. Nothing
was left out, and `Drums.wav` is the FLAC decoded by ffmpeg.

- **The `.mid`** is `garageband.expected.mid` event for event. That file is
  `crates/session`'s fixture, written by `make_import_bundle.py`'s rewrite.
  The one difference is the voice's name: *V*, where the fixture has
  *Voice*. `partsOf` names a part from its track when the playback map has
  no staff for the words or the drums, as the Parts fold does
  (`garageBandPlan.test.ts` says so).
- **The WAVs.** The recording, cut by 1.3451 s, and `Bass.wav`, padded by
  1.0549 s, are frame for frame the spike's `Song.wav` and `Bass.wav`.
  `Drums.wav` differs only in its first 24 575 frames, where the spike's
  file has a count-in click before tick 0 and a stem on the score's time
  has none. So each WAV's sample 0 is the MIDI's tick 0, the offset of
  −2.40 s included.

**In GarageBand [measured], from the save.**

1. `open -b com.apple.garageband10` on the `.mid`, as stage 3 runs it, made
   *Untitled 11*.
2. `save document 1 in` wrote it to a scratch folder.
3. `close document 1 saving no` closed it, and nothing was written to
   `~/Music/GarageBand`.

`inspect_band.py` and `inspect_band.py --links` read the save:

| What | Arrived | How it shows |
| --- | --- | --- |
| **A track per part** | yes, and **no control or words track** | `NumberOfTracks` 6, the regions *V*, *Violin*, *Cello*, *Piano · G clef*, *Piano · F clef* and *Drums*: `Lean.mid`'s result in §4.3 |
| **Part names** | on the regions, UTF-8 intact | As §4.3 found, the one-letter *V* too. `inspect_band.py` printed that as *(none)*: its `strings` needs two letters. It now reads an `MSeq`'s name where it is kept, at body +0x10 |
| **Programs** | yes | 52 → *Classical Ensemble*, 40 and 42 → *String Ensemble*, 0 → *Steinway Grand Piano* on both staves, and the drums → *SoCal* |
| **Tempo map** | yes, to the hundredth | 113.76 at q+0 and 85.32 at q+15 (bar 6). `BeatsPerMinute` is 113.76 |
| **Meter and key** | yes | 3/4 and G major |
| **The lead-in** | yes | The pickup (voice, violin, hi-hat) is at q+2, beat 3 of bar 1. The score's bar 1 is at q+3, GarageBand's bar 2 |
| **CC 7 and CC 10** | none | The file has none to bring |
| **The links** | every check held | `--links` exits 0. There is no new report in `~/Library/Logs/DiagnosticReports/` |

**Left for the operator.** These need eyes and ears:

- the drop and listening of §4.4, here with a negative offset and a stem
  decoded from FLAC, though the recording is still synthesized;
- the panel, the alert and stage 3's opening, by eye.

The steps:

1. Close any project in GarageBand.
2. Run the Mac app (`npm start` in `packages/studio`) and open
   `Lily Import Test.ly`.
3. Choose *File › Export for GarageBand…* and then its folder. Answer
   *Replace* if the alert asks.
4. GarageBand opens `Lily Import Test.mid` as a new project, with six
   tracks.
5. In the Finder, select the three WAVs of `Lily Import Test (GarageBand)`
   and drag them together into the empty area below the tracks, at the
   start of bar 1. Three audio tracks should appear, *Song (recorded)*,
   *Bass* and *Drums*, each at bar 1.
6. Play from bar 1. Listen for:
   - the clicks, in *Drums* and *Song (recorded)*, sounding with the SoCal
     kit throughout, across the change to 85.32 at bar 6;
   - each bass note starting with its bar's MIDI notes from bar 2;
   - the pickup on beat 3 of bar 1.
7. Close the project without saving.

To rebuild that score folder: run `make_import_bundle.py`, then take
`work/score.ly` and `work/Song (recorded).wav`. Cut the lead-in (2 × 60 ÷
113.76 s) from the head of the bundle's `Drums.wav` and `Bass.wav`, and
name them as above, the drums converted to FLAC with `afconvert`. Then add
the two lines to the score.

## 9. Open-source work on the format: a survey

§9 to §13 ask whether Lily Studio could write a `.band` itself, not only
patch one (§5). This section surveys the public code that reads or writes
the format, and §9.6 sets out how LogicProFormatWriter (LPFW) links the
records it writes. §10 makes projects one change apart and diffs them, §11
decodes the links from them, §12 tries LPFW's output in GarageBand, and §13
builds projects of a new shape. What the projects claim is marked as a
claim; what was run against GarageBand 10.4.14 on 2026-11-03 is
**[measured]**.

**In short:**

| Question | Answer | Where |
| --- | --- | --- |
| Does public code write GarageBand's `ProjectData`? | Barely. gb-in-Claude (MIT) adds audio regions and files to a project GarageBand saved, and **claims** GarageBand opens it. It has GarageBand make every track. Several projects read GarageBand's files. The Logic writers, LPFW (MIT) and logicxkit (Apache-2.0), were tried by their authors in Logic only | §9.2, §9.3 |
| Which code could come into the MIT apps? | MIT and Apache-2.0 code with its notice: LPFW, logicxkit, gb-in-Claude, wavelength, logic2ableton and others. GPL code may only be read. Code with no licence may not be used | §9.5 |
| How do the records link? | By index, slot or number, never by offset: track row → `Envi` → `AuCO`; placement → row, and region `MSeq` or `AuFl` and `AuRg`. **[measured]** on 31 projects | §11 |
| What else must a writer keep up? | The sequence counters, the arrange's height, `NumberOfTracks`, the strip numbers and `AuCn` 9's counts, and the `Song`'s registry, two sorted tables of every object. Names padded to even. **[measured]** | §11.3, §11.4, §13.2 |
| What is still opaque? | The plug-in state, the counter's rule, two fields of a placement, the `Song`'s other tables, a Drummer track's links | §11.6 |
| Does GarageBand open LPFW's output? | **No [measured].** It declines LPFW's projects or stops at an *OSStatus error -19* alert, and refuses Logic 12.2's own save at GarageBand's format code. A `ProjectData` GarageBand wrote opens in the same package | §12 |
| Can a project of a new shape be written? | **Yes [measured]**, from a project GarageBand saved: 14 tracks, 17 MIDI regions and 7 files from a base of 2 tracks. GarageBand opens it with no dialog, plays it, and saves it with every link intact | §13 |
| What does that not show? | Which of the bookkeeping GarageBand needs, a new track with another instrument than the base's, and any other base or GarageBand version | §13.5 |

### 9.1 How it was gathered

- **Where.** On 2026-11-03:
  - web searches;
  - GitHub repository and code search: the tags `qeSM`, `karT`, the magic
    `0xabc04723`, `Alternatives/000/ProjectData`, `LogicProFormatWriter`,
    `com.apple.garageband.project`;
  - GitLab, Codeberg, PyPI (by name), crates.io and npm.
- **How it was read.** Each project was cloned at its newest commit into its
  own empty folder outside the repository and read there. Nothing was built,
  installed or run. Each licence is the one in its `LICENSE` file, checked
  against its package metadata. Fixture versions are the
  `LastSavedFrom` of each fixture's `ProjectInformation.plist`, and format
  codes are bytes 4 and 5 of its `ProjectData`.
- **What it says.** What a project's own code or notes say about itself
  (*claims*) is marked as such. A *link* is how one record points at
  another (§2.6).
- **What was left out:**
  - **Three look-alike projects that were not opened.** They call
    themselves a *Windows and macOS helper for Garageband data paths*,
    under three accounts: `carolyn-parker23/garageband` on GitHub (since
    removed), `julia108334/garageband-desktop` on GitLab and
    `vshea242/garageband-desktop` on Codeberg. GarageBand runs on no
    Windows.
  - **The Logic Pro Help thread *Internal Logic Pro X file format docs?*.**
    The server refused the request (HTTP 403).
- **Read but not to be used.**
  - **logic-pro-ctl** works from static analysis of Logic's executable,
    and **logicparse** takes its field types from a Frida script run on
    Logic. Both rest on the app's code, not on its files (§7.2).
  - **band2flp** used IDA too, but only as a hex viewer on the song's data,
    with no code loaded (its `EXP-001` and `midi-preview-timing.md`).

### 9.2 Projects that read or write GarageBand's `ProjectData`

| Project | Language, licence, last commit | Reads or writes | What it decodes | Versions | With GarageBand |
| --- | --- | --- | --- | --- | --- |
| [gb-in-Claude](https://github.com/rafaqat/gb-in-Claude) (`src/band/`) | TypeScript, MIT, 2026-10-07 | **Writes.** Adds records to a project GarageBand saved: `AuFl`, `AuRg` and `GenM` for each new audio region, and an 80-byte placement event in the arrange's `EvSq`. The track must exist already: it is made through GarageBand's UI (Accessibility), and the MIDI through GarageBand's own import | visible tracks (§9.5); a `Trak`'s strip and an `Envi`'s kind and name; placements; the `AuFl` chain; each region's 16 identity bytes; notes, programs, controllers, bends | `0x09d0`; five fixtures saved by 10.4.14 | **Claims** that a project with an audio region added to an empty audio track opened with no dialog, and that GarageBand's re-save kept the region. Not repeated with its code, but §13 adds audio regions and files the same way with its own, and GarageBand opens them **[measured]**. The new region's bytes come from a template cut from a GarageBand save (`audio-slot-template.ts`) |
| [wavelength](https://github.com/austinginder/wavelength) (`src/garageband.cpp`) | C++, MIT, 2026-10-19 | **Reads** a `.band` into its own JSON job. It writes a synthetic `.band` only to test its own reader (`scripts/make-test-band.py`) | the deepest reader of GarageBand 10: tracks, channel strips (`AuCO`: fader, pan, mute, solo), plug-ins and Audio Unit state, sends, automation, notes, Drummer regions, audio regions and loops, tempo, meter, key, markers | *10.2 and 10.4 saves* (its header comment) | read side only. No sign that GarageBand was given its test file |
| [Band2Midi](https://github.com/tomoutsuki/Band2Midi) (`band2midi/parser.py`, `docs/format.md`) | Python, **GPL-3.0**, 2026-09-20 | **Reads** GarageBand for iOS projects: the song inside the top-level `projectData` (§2.4) | a structured walk. Tracks, regions, the region windows, notes, a constant tempo, 4/4 only. It decodes the links of §9.5 | GarageBand for iOS 2.3.19 only. It refuses any header but `23 47 c0 ab cf 09 …` | 18 iOS fixtures (not in the repository) |
| [band2flp](https://github.com/veexctrl/band2flp) (`band2flp/parser.py`, `docs/`) | Python, MIT, 2026-11-01 | **Reads** zipped iOS projects and converts them to FL Studio | a walk, events, audio and MIDI placements, tempo, meter, all as rated *candidates*. `docs/reverse-engineering/` logs over 100 experiments, many on links | GarageBand for iOS 2.3.19 | two private fixtures |
| [band-to-midi](https://github.com/zhafen/band-to-midi) (`gb_to_midi.py`) | Python, MIT, 2026-05-25 | **Reads**: Mac projects, both the old base64 `projectData` and `ProjectData`, to MIDI. It scans for the `qSvE` tag rather than walking the records | notes, tempo | two fixtures saved by **10.4.14** (`0x09d0`), each with the MIDI it came from | its tests compare the two |
| [bandzip-extract](https://github.com/mizumoto-cn/bandzip-extract) | Python, no licence, 2026-08-11 | **Reads** zipped iOS projects; scans for tags | piano notes, tempo, meter | *tested piano projects* | none in the repository |
| [garageband-mcp](https://github.com/garageband-mcp/garageband-mcp) | TypeScript and Swift, no licence (`"private": true`), 2026-08-13 | drives GarageBand through Accessibility. Its `ProjectData` tools only hash it, sort its type and search it as text. Its `experimental_patch` replaces text in the file as UTF-8, which would corrupt the binary. It copies the app's templates as they are | nothing | 10.4.14 | hand runs **claimed** |

**Around GarageBand, but not its `ProjectData`:**

- **garageband-llm-bridge** ([extao15](https://github.com/extao15/garageband-llm-bridge), MIT) and **`@xuehanqian/garageband-ai`** (npm 0.2.0, MIT) write MIDI and hand it to `open -a GarageBand`.
- **garageband-stem-export** ([amonbenson](https://github.com/amonbenson/garageband-stem-export), MIT) exports stems through the UI.
- Three tools read only the package's plists and media:
  - **human-music** ([KainanYuval](https://github.com/KainanYuval/human-music), no licence);
  - **logiclens** ([TorrieDigital](https://github.com/TorrieDigital/logiclens), no licence);
  - **iMedia** ([karelia](https://github.com/karelia/iMedia), a modified MIT licence): `Output/metadata.plist` of the old layout.
- **ZIP2GB** ([retro-tsuki](https://github.com/retro-tsuki/ZIP2GB), no licence) only re-zips a file.
- **GB2MIDI** ([larkob](https://github.com/larkob/GB2MIDI)) and **extract-garageband-midi** ([mrcoles](https://github.com/mrcoles/extract-garageband-midi)) take MIDI from an Apple Loop `.aif`, not from a project. Neither has a licence.
- **garageband-research** ([SatyrDiamond](https://github.com/SatyrDiamond/garageband-research), MIT) holds only a licence.
- **DawVert**, by the same author, has no GarageBand or Logic plug-in.

### 9.3 Projects that write or edit Logic's `ProjectData`

| Project | Language, licence, last commit | How it writes | What it decodes | Versions | GarageBand |
| --- | --- | --- | --- | --- | --- |
| [LogicProFormatWriter](https://github.com/jonkubis/LogicProFormatWriter) | Python, MIT, 2026-10-29 | builds sessions from **seeds** cut from sessions Logic saved (`logicx/data/`, `bake_seeds.py`, `DONORS.md`). A seed is a base session with a pre-allocated mixer of 64 or 1000 strips, and new tracks take its free strips. Region and track records are cloned from prototypes. §9.6 reads it in depth | §2.7, plus take folders | 113 fixtures saved by Logic 11.2.2 (`0x09cf`), 2 by Logic 12.2 (`0x09d0`). `DONORS.md` says the donors were made in Logic 12.0.1 | **refused** by GarageBand 10.4.14, as are its Logic fixtures (§12) **[measured]** |
| [logicxkit](https://github.com/phierceweb/logicxkit) (PyPI 0.15.0) | Python, **Apache-2.0** with a `NOTICE`, 2026-11-01 | **edits in place and adds records.** It adds a track the way Logic does: clones an `Envi` object, adds the arrange and mixer rows, a sequence triple, an index-table entry and the `gnoS` registry entries, then renumbers everything after them (`services/arrange/addtrack.py`, `stream/registry.py`, `stream/table_index.py`). It also renames and reorders tracks, makes stacks, and edits routing, sends, plug-ins, automation, MIDI, regions, tempo and markers | the fullest public model of how Logic's records link, including the `Song` registry and the index table | Logic 12.3.1 and 12.4 (`0x09d1`, 400 corpus files) | none |
| [loov/logicx](https://github.com/loov/logicx) | Go, **GPL-3.0**, 2026-10-11 | reads the file as a tree. Its `Save` methods write decoded values back in place, and it can delete and duplicate events. It exports MusicXML | tracks, strips, plug-ins, tempo, signatures, notes, chords, audio files, regions and placements | Logic 12.2 (`0x09d0`, 25) and 12.4 (`0x09d1`, 46) | none |
| [chimera](https://github.com/jeffehobbs/chimera) | Swift, no licence, 2026-09-12 | rewrites and splices `ProjectData`: tempo, meter, tracks grafted from another project | header +0x08 as an index shifted left by 18, as in §2.5 | **claims** that its output opens in Logic 12.3.1. Grafted tracks arrive *without their instruments*, since no mixer records come with them | none |
| [daw2logic](https://github.com/audiohacking/daw2logic) | Python, MIT in its README and `pyproject.toml`, **no `LICENSE` file**, 2026-06-07 | DAWproject to `.logicx`, through LogicProFormatWriter's `synthesize_av_region_bundle` (a submodule), plus its own mixer code | LogicProFormatWriter's | LogicProFormatWriter's | none |
| [Dawconverter](https://github.com/jpphillippe/Dawconverter) | Python, MIT in `pyproject.toml`, no `LICENSE` file, 2026-09-29 | a fork of daw2logic that also reads `.logicx` back to DAWproject | the same | the same | none |
| [amt-logic-companion](https://github.com/terryTM/amt-logic-companion) (`python/inject_midi.py`) | Python, MIT, 2026-10-02 | writes notes into an existing `EvSq`. It makes the track by scripting Logic's menus | notes | — | none |
| [show-file-skills](https://github.com/michaelkeithlewis/show-file-skills) (`apple/logic/`) | Python, MIT, 2026-11-02 | adds an `AUSampleDelay` insert to channel strips | strips, inserts, routing, Audio Unit state; not regions | Logic 11 and 12 | none |
| [LogicFiles](https://github.com/CraigStuntz/LogicFiles) | Swift, MIT, 2026-04-27 | writes `.cst`, `.patch`, `.pst`, `.aupreset` and a `.logicx`'s plists. It keeps `ProjectData` as opaque bytes | plists and channel-strip files | — | none |
| [logicparse](https://github.com/Antonabi/logicparse) | Python, no licence, 2026-08-09 | `Song` and `MSeq`; its own README calls writing *extremely buggy* | **instrumentation**: field types from Frida (§9.1) | — | none |

### 9.4 Projects that only read Logic's `ProjectData`

| Project | Language, licence, last commit | What it reads | Versions |
| --- | --- | --- | --- |
| [logic2ableton](https://github.com/Evilander/logic2ableton) (`logic2ableton/logic_project_data.py`) | Python, MIT, 2026-10-14 (PyPI 2.5.1) | a structured walk: events, notes, markers, tempo, meter, region placements, `MSeq` windows, `Envi` objects, `AuCO` strips (type, number, fader, pan, mute, solo, input, output), `AuFl`, `AuRg`, take folders. Its module comment states the links of §9.5 | Logic 10 and 11 on (a header field of 1 for Logic 10, 2 from Logic 11) |
| [BetterALMAMPlayer](https://github.com/Prod-DJD/BetterALMAMPlayer) (`scripts/core/logic_reader.gd`) | GDScript, GPL-3.0, 2026-11-02 | MIDI tracks, regions and notes. A port of LogicProFormatWriter's (MIT) reading rules, **claimed** to match Logic's own MIDI export note for note | — |
| [logicvisualizer](https://github.com/blix5/logicvisualizer) (`src/main/logic/`) | TypeScript, no licence, 2026-09-22 | the arrange track list with stacks, MIDI regions, tempo, markers. Vendored, read half only, from a private app whose write half adds regions (`VENDORED.md`) | 57- and 58-byte `Trak` bodies |
| [f-fritz/logicx-parser](https://github.com/f-fritz/logicx-parser) | Python, no licence, 2026-08-15 | a record walk: the tempo map, meter, key, sample rate, track count, audio files | Logic 12.2 (its example) |
| [cigol](https://gitlab.com/fastfourier666/cigol) (GitLab) | Python, no licence, 2022-04-06 | scripts that dump and diff records and the environment; notes on the headers | Logic 10.4.8, 10.5.1, 10.7.1, and GarageBand 10.3.4 (its notes) |
| [lpx_parser](https://github.com/johndpope/lpx_parser) | Python, MIT, 2025-05-11 | a generic record dump and diff, with one sample file | — |
| [logicx-analyzer](https://github.com/geoffmyers/logicx-analyzer) | Python, GPL-3.0, 2026-10-24 | tag markers and strings: plug-ins, Session Player presets, audio files, tempo candidates | — |
| [lpx-explorer](https://github.com/rhydlewis/lpx-explorer), [lpx-toolkit](https://github.com/rhydlewis/lpx-toolkit) | Rust and TypeScript, Python, GPL-3.0, 2026-08-31 and 2026-05-08 | byte patterns, not a record walk: plug-ins, tracks by their name fields, region names, the track registry. **lpx-toolkit 0.2.1 on PyPI says MIT**, while its `LICENSE` and `pyproject.toml` say GPL-3.0-or-later | — |
| [bravoh-daw](https://github.com/ozzaii/bravoh-daw) (crates.io 0.1.0) | Rust, MIT, 2026-07-11 | strings only: plug-in chains, samples, markers, routing | — |

**Not about the record format:**

- [logician-mcp](https://github.com/BanneBanning/logician-mcp) (MIT) watches only `ProjectData`'s modification time.
- [daw_buddy](https://github.com/hrdsht/daw_buddy) (MIT) indexes bundles.
- [logicx-mcp](https://github.com/audiohacking/logicx-mcp) (MIT or Apache-2.0) is about Audio Units.
- [clogic](https://github.com/brandonapol/clogic) (Apache-2.0) is a research note.
- [Pro-Wave](https://github.com/fabox-os/Pro-Wave) (GPL-3.0) has a spec for importing `.logicx` but no code for it.
- [logic-pro-ctl](https://github.com/uniuninaruru/logic-pro-ctl) (MIT) works from Logic's executable (§9.1).
- On npm, `ununu-logic-parser` reads the old `.logic` format.

### 9.5 What it adds to §2

These come from the code and notes of the projects above. §11 to §13
checked most of them against GarageBand's files, and each says what was
found.

- **Format codes, from the fixtures' plists:**

  | Code | Saved by | Source |
  | --- | --- | --- |
  | `0x06eb` | Logic 10.4.8 and GarageBand 10.3.4 | cigol's notes |
  | `0x07d0` | Logic 10.5.1 | cigol's notes |
  | `0x09c5` | Logic 10.7.1 | cigol's notes |
  | `0x09cf` | Logic 11.2.2 | LogicProFormatWriter's fixtures |
  | `0x09cf` | GarageBand for iOS 2.3.19 | the header Band2Midi requires |
  | `0x09d0` | Logic 12.2 | LogicProFormatWriter and loov/logicx |
  | `0x09d0` | GarageBand 10.4.14 | §2.7, gb-in-Claude, band-to-midi |
  | `0x09d1` | Logic 12.3.1 and 12.4 | logicxkit and loov/logicx |

  So **GarageBand 10.4.14 writes the code of Logic 12.2, not of Logic
  11.2.2** as §2.7 first had it. The specification's own line, *`D0 09 / CF
  09` (Logic 11.2.2)*, does not tell them apart. §12.2 read LPFW's
  fixtures again: `0x09cf` from Logic 11.2.2 and `0x09d0` from Logic 12.2
  **[measured]**.
- **The links, as four or more projects read them independently.** The
  readers name the same header bytes differently: a u32 *group* at +0x08
  (gb-in-Claude, band2flp), or a u16 class at +0x06 and a u32 object id at
  +0x0A (wavelength, Band2Midi). Both are §2.5's *index shifted left by
  18*.
  - **The arrange** is the sequence of class 23 (0x17), id 4: its `MSeq`
    and its `EvSq`. (Band2Midi, wavelength, loov/logicx, logic2ableton).
    **[measured]** (§11.1): class 23, §2.5's index 1.
  - **The tracks** are the arrange's `Trak` records, in order. A `Trak`'s
    u32 at body +0x08 names its channel strip's `Envi` object.
    gb-in-Claude finds that `Envi` in group *strip* << 16. It counts a
    `Trak` as a visible track when the low half of its first u32 is 1, and
    takes a high half of `0x0014` to mean the track holds regions.
    (Band2Midi, gb-in-Claude, wavelength, loov/logicx). **[measured]**
    (§11.2): the u32 at body +0x06 is the `Envi`'s index, and body +0x08
    its slot. The low half is the row's kind (1 a track, 3 the master, 4 a
    Drummer track), and 0x14 marks a row with an event in its folder.
  - **An `Envi`'s name** (u16 length at body +0x9E, as in §5.2) is followed,
    at an even offset, by its channel strip's number plus one. That number
    counts through the strip types in a fixed order, not by the strip's
    place in the file. (wavelength, logic2ableton). **[measured]**: the
    number plus one holds (§11.2). In GarageBand's files the number counts
    the `AuCO`s in file order (§11.3), and the strips sit grouped by kind
    (§13.2), so the two readings agree there.
  - **A region's placement** is an 80-byte event in the arrange's `EvSq`:
    `0x20` for MIDI, `0x24` for audio. It holds:
    - its position at +0x04;
    - the track's `Envi` at +0x10;
    - the track's number, or lane, at +0x14;
    - for MIDI, the region's `MSeq` id at +0x20;
    - for audio, the region and the file at +0x28 and +0x2C (gb-in-Claude
      writes *region × 4* at +0x2C).

    (Band2Midi, gb-in-Claude, logic2ableton, band2flp). **[measured]**
    (§11.2, §11.5): all hold, with +0x10 the row's slot. +0x2C is the
    file × 4 and +0x28 the region within the file, so gb-in-Claude's
    *region × 4* is wrong once a file has two regions.
  - **A region's `MSeq` and `EvSq`** share a class and id. The `MSeq` holds
    the region's name and the window it plays. (Band2Midi, wavelength).
    **[measured]** (§11.1).
  - **An audio file and its regions:** `AuFl` and `AuRg` share a group.
    gb-in-Claude gives region *n* the group *n* × 0x40000 and its `GenM`
    the next group up. It also chains the `AuFl` records through two fields
    after `EVAW`. logic2ableton reads an `AuRg`'s first id as its `AuFl`'s.
    **[measured]** (§11.2): they share an index. No `AuRg` id appears in
    another record, so in GarageBand's files the index is the only link.
    The chain was not checked.
  - **Logic's bookkeeping.** Logic also keeps a registry of objects in
    `Song` and an index table (logicxkit). Logic drops a track whose slot
    the registry lacks, on load. **[measured]** (§11.3, §13.2):
    GarageBand 10.4.14's `Song` has a registry too, two sorted tables of
    every object of classes 1 to 33. §13 keeps it up. Whether GarageBand
    drops what it lacks was not tried.
- **Adding records has been done.**
  - **In GarageBand:** gb-in-Claude adds whole audio regions and files to
    projects that GarageBand 10.4.14 saved (claimed, §9.2). It still lets
    GarageBand make each track.
  - **In Logic:** logicxkit adds tracks the way Logic does, and
    LogicProFormatWriter fills pre-allocated strips. Both claim Logic opens
    the result.

  No project here builds a GarageBand project of a new shape. §13 does,
  from a project GarageBand saved **[measured]**.
- **Licences for reuse in the MIT apps (§7.2):**
  - **Usable with their notice:** LogicProFormatWriter, gb-in-Claude,
    wavelength, band2flp, band-to-midi, logic2ableton, lpx_parser,
    show-file-skills (MIT), and logicxkit (Apache-2.0, with its `NOTICE`).
  - **Readable only:** the GPL projects: Band2Midi, loov/logicx,
    logicx-analyzer, lpx-explorer, lpx-toolkit, BetterALMAMPlayer,
    Pro-Wave.
  - **Not reusable at all:** projects with no licence, including chimera,
    cigol, logicvisualizer and garageband-mcp. daw2logic and Dawconverter
    claim MIT but ship no `LICENSE` file.
  - **Apple's bytes.** gb-in-Claude's region template and
    LogicProFormatWriter's seeds are bytes that GarageBand or Logic
    wrote, as a template would be (§7.2).

### 9.6 How LogicProFormatWriter links records (per LPFW)

This is how
[LogicProFormatWriter](https://github.com/jonkubis/LogicProFormatWriter)
(LPFW) numbers, links and counts the records it writes, as its spec and
code have it. It is LPFW's model of Logic's files, not a measurement.
§11.5 checks it against GarageBand's files, and most of it holds. §12 runs
LPFW, and GarageBand opens none of its output.

- **What was read.** Commit
  [`9935ca8`](https://github.com/jonkubis/LogicProFormatWriter/tree/9935ca88a3fad4c4232eb9dd755149e9cc9b9b65)
  (2026-10-29), in a fresh clone:
  - `PROJECTDATA_FORMAT.md`: §2, §3, §8 to §8.5, §10 to §10.9, §13, §18
    to §22 and §41;
  - in `logicx/`: `projectdata.py`, the writers, and `session.py`, the
    reader;
  - `bake_seeds.py` and `DONORS.md`.

  For this section its code was not run; §12 runs it. The seeds in
  `logicx/data/` were read as data by a standard-library script of our own,
  run with `python3 -I`.
- **Names.** Tags are written forward, as in §2.5. LPFW's reversed tags
  map onto them: `karT` Trak, `ivnE` Envi, `qeSM` MSeq, `qSvE` EvSq,
  `gRuA` AuRg, `lFuA` AuFl, `gnoS` Song, `OCuA` AuCO, `UCuA` AuCU, `MneG`
  GenM, `OgnS` SngO, `tSxT` TxSt, `lytS` Styl.
- **Offsets.** *rec +* counts from a record's first byte, header
  included. *body +* is rec + 0x24. *event +* counts from an event's first
  byte.
- **Indexes.** An index is given as the u32 at rec +0x08, which is §2.5's
  index shifted left by 18. So `0x040000` is §2.5's index 1, and §9.5's
  *id 4*.

#### 9.6.1 Nothing is built from nothing

LPFW never builds a record from scratch. Every record it writes is either
a seed record changed in place or a clone of a record that Logic wrote.
Its own reason (spec §10.6.1): a new channel makes Logic regenerate ids
and renumber the whole `AuCO` block, which is impractical to reproduce.
Deleting tracks keeps the mixer, though. So a seed is a project that once
had N tracks and now has one. Its N − 1 free channel strips can each be
turned into a track.

| Seed (`logicx/data/`) | What it is | Saved by, format | Records |
| --- | --- | --- | --- |
| `audio_base.seed` | 64 audio tracks, 63 deleted | Logic Pro 11.2.2 (6387), `0x09cf`, form 2 | 856: 440 AuCO, 151 Trak, 79 MSeq, 79 EvSq, 12 Envi |
| `audio_base_xl.seed` | the same with 1000 | the same | 5 536: 1 376 AuCO, 2 023 Trak, 1 015 MSeq |
| `mixed_base.seed` | 1 software-instrument and 1 audio track | the same | 919: 503 AuCO, 153 Trak, 79 MSeq, 13 Envi |

`DONORS.md` says the donors were made in Logic 12.0.1. Each seed's
`ProjectInformation.plist` says Logic Pro 11.2.2.

`infra.json.gz` holds records that `bake_seeds.py` cut from Logic saves
that differ by one change:

- **For a project's first instrument or audio track** (`mixed_base` + 1
  instrument, + 2 instruments, + 1 audio):
  - the new channel's Envi (505 or 507 bytes);
  - two AuCU of plug-in state (1 957 and 867 bytes);
  - a *trio* at index `0x4000000`: an MSeq of 341 bytes, a Trak of 36 and
    an EvSq of 52;
  - a grown AuCO of 241 bytes, which replaces the seed's 221-byte one.
- **A MIDI region prototype** (`F23_av`): five records (TxSt, Styl, MSeq,
  Trak, EvSq) and its 80-byte `0x20` placement event.
- **An audio region prototype** (`F21`): an AuFl (638 bytes), an AuRg
  (242 bytes) and its 80-byte `0x24` placement event.

The rest of a project is the seed's, byte for byte. That covers the Song,
every AuCO, the CoreMIDI tables and the score styles, as well as
`MetaData.plist` and `ProjectInformation.plist`.

#### 9.6.2 How each record is made

| Record | What LPFW does | Copied from |
| --- | --- | --- |
| **Envi** (a channel) | Clones the highest channel. For the first instrument or audio track on the mixed seed, it clones the reference Envi instead. Sets: the index at rec +0x08; the slot at rec +0x34 (index >> 16); a fresh 16-byte channel UUID; the last-channel flag at rec +0x74. The name (u16 length at rec +0xC2, text at +0xC4: §9.5's body +0x9E) is replaced last, which resizes the record. The track shows this name. | seed, or `infra` |
| **Trak**, arrange row (index `0x040000`) | Clones the seed's row for channel `0x580000`, the first audio track. Sets the channel at rec +0x2A, the ordinal at rec +0x12, a fresh id at rec +0x3C, and 0x20 at rec +0x4C (the newest row; cleared on the others). Inserts it before the master's row, whose ordinal goes up by one. | seed |
| **Trak**, per-slot rows (index `0x080000`) | Not added. Each row holds a slot at rec +0x2C and a rank at rec +0x12. Ranks are recomputed and the rows are re-sorted by rank. | seed |
| **MSeq**, arrange container (`0x040000`) | Only its track-area height changes: the u16 at name end + 0x0A, set to 0x3C × (tracks + 1). | seed |
| **MSeq**, MIDI region | Cloned from the prototype at a new index. The channel it names is re-stamped: a u32 holding the channel's index, at name end + 0xCA (rec +0x106 for the name *Inst 1*). Also set: start (zero-based ticks) at rec +0x11C, length at rec +0x78, and the name (u16 length at rec +0x34), which resizes the record. | `infra` |
| **EvSq**, arrange (`0x040000`) | Holds every region's 80-byte placement event, audio (`0x24`) and MIDI (`0x20`) together, before the 16-byte trailer `f1 00 00 00 ff ff ff 3f …`. Each event is cloned from the prototype's, and position, track and link are set (§9.6.3). | events from `infra` |
| **EvSq**, MIDI region | Built: one 32-byte note event per note (§2.6), then the trailer. | built |
| **AuRg** | Cloned from region 0 of the prototype, at index *i* << 18. Its UUID at rec +0xCA gets random bytes 0–3 and 8–15, and keeps bytes 4–7. Its length in frames goes at body +0x16 and its name at body +0x4A, which resizes the record. | `infra` |
| **AuFl** | Cloned the same way. The UTF-16 file name at body +0x08 is resized. Fields counted from the `EVAW` tag: frames +0x0C, rate +0x14, channels +0x18, bits +0x1A, the file's size −0x32, and the folder path −0x13E, set to the relative `Audio Files`. | `infra` |

Other records:

- **AuCO.** A pre-allocated strip is claimed (§9.6.4).
- **AuCU and the trio.** Copied whole for a project's first instrument or
  audio track.
- **GenM.** It gains one Drummer JSON entry per track.
- **SngO.** The base's audio pool is kept as it is, or emptied to a fixed
  32-byte body. Per-region GenM records are dropped. Logic, the spec says,
  rebuilds both on load.
- **`MetaData.plist`.** `NumberOfTracks` must equal the tracks, or Logic
  rejects the project (spec §10.6.6). `AudioFiles` lists
  `Audio Files/<name>`, and `SampleRate` is set.
- **WAVs.** Each one gets an `LGWV` chunk if it lacks one. Without it,
  Logic rewrites the file, its size no longer matches AuFl's, and the
  region drops out of the audio list.

#### 9.6.3 Indexes and links

**Indexes** (rec +0x08, in steps of `0x40000`):

| What | Index |
| --- | --- |
| Song | `0xFFFFFFFF` |
| Channels (Envi) | slot << 16. The slot is a multiple of 4 and unique. System slots run from 0x00 to 0x54: Output at 0x50 and Master at 0x54 (spec §19). The first audio track is `0x580000`. A new channel takes Song rec +0x80 + `0x40000` |
| Master and global Trak rows | `0x000000` |
| Arrange MSeq, EvSq and track rows | `0x040000`. A settled mixed base has a second, larger MSeq here, named *Untitled*, whose height is 0. LPFW calls it a decoy: writing its height corrupted the file. The real container is the one whose height is a non-zero multiple of 0x3C, and the arrange EvSq is the first EvSq at `0x040000` after it |
| Per-slot Trak rows; the arrange-order EvSq | `0x080000` |
| Every AuCO | `0x240000`, a type tag. The ordinal << 16 is at rec +0x0C |
| Audio file *i* | AuFl and its AuRg records at *i* << 18. Region *k* of the file has *k* << 16 at rec +0x0C |
| MIDI region *j* | its five records at `0x1200000` + *j* × `0x40000`, an unused high range |

**Links**, each as LPFW writes or reads it:

| From | Field | To |
| --- | --- | --- |
| Trak, arrange row | rec +0x2A, u32 | the Envi channel's index. The u16 at rec +0x2C (§9.5's body +0x08) is the slot |
| Trak, arrange row | rec +0x3C, 16 bytes | a key in GenM's `drummerModelTrackStates`, written as a UUID string. Without it, LPFW finds the track is not drawn |
| Envi | channel UUID | AuCO rec +0xBD, which must be equal. Otherwise the strip has no fader, pan or plug-ins |
| Trak, per-slot row | rec +0x2C | a channel slot. Logic reads these rows in file order as the track list |
| Placement event | event +0x14 (u16) | the track: its 1-based place among the arrange rows, in file order, master left out |
| Placement event | event +0x10 | the channel slot (MIDI). Audio keeps the prototype's 0x58, which LPFW found makes no difference |
| `0x20` event | event +0x20 | the MIDI region's index >> 16 |
| `0x24` event | event +0x2C | audio file *i* × 4 |
| `0x24` event | event +0x28 | region *k* within that file. Left as the prototype's 0 |
| MIDI region MSeq | name end + 0xCA | the instrument channel's index |
| AuRg ↔ AuFl | the same index | no field links them. LPFW keeps them next to each other, as Logic writes them |

Placement position is 34560 + ticks, at event +0x04. A MIDI region keeps
its start a second time in its MSeq, zero-based. Its notes count from
38400, from the region's start.

#### 9.6.4 Counters and bookkeeping

What LPFW sets on each added track:

- **Song.**
  - rec +0x80: the highest channel index in use.
  - rec +0xF4: tracks << 16.
  - rec +0xF8: (tracks << 16) | 1.
- **Song's registries.** There are four per-channel tables, 80 bytes per
  pre-allocated channel in all. The first holds 24-byte rows
  `[u32 0x14][u32 slot][UUID]` from rec +0x1E20. The 64- and 1000-track
  seed path fills a new channel's rows in the two tables tagged 0x14,
  and refreshes two time stamps in a table tagged 0x17. The mixed
  path writes only the counters above, because Logic regenerates the
  registry on load (spec §10.9). When a seed runs out of rows,
  `grow_channel_pool` appends free rows and free AuCO strips.
- **The arrange's height.** The u16 at name end + 0x0A of the arrange MSeq
  is set to 0x3C × (tracks + 1). If it is too small, Logic draws only as
  many tracks as fit.
- **Ranks.** Per-slot Trak ranks at rec +0x12, then a re-sort by rank. Set
  without the re-sort, no track is drawn.
- **The arrange-order table.** The EvSq at `0x080000` holds 0x50-byte rows
  from rec +0x38, and each row's first byte is an ordinal.
- **Envi.**
  - The last-channel flag at rec +0x74 moves to the new channel.
  - rec +0x76 goes up by 0x42 on the new channel. On the mixed path it
    also goes up on other channels: on the buses, outputs and master
    when an instrument is added, and on every channel from `0x480000` up
    that is not an audio track when an audio track is added.
  - rec +0x51 goes up by 2 on the channel that was last, except when an
    audio track is added on the mixed path.
  - On the audio seed path, the bytes at rec +0xCA, +0xCC and +0xCF go up
    by 1. The mixed path leaves them, because bumping them there left the
    strip empty.
- **AuCO.** It takes a free strip, one whose UUID is zero. For an audio
  track it prefers rec +0x82 = the track count, then 0xFF. An instrument
  takes any free strip marked `29 f5` at rec +0x70. It sets rec +0x3C and +0x3D to 1,
  rec +0x82 to 0, and the UUID. Stereo is three bytes: rec +0x72, +0x7A
  and +0x9F.
- **GenM.** A JSON entry is inserted, and the u32s at rec +0x24 and +0x40
  and the record's length grow by its size.
- **Fresh ids.** 16 bytes: 4 random, 4 fixed per session, 8 random. On the
  audio seed path the fixed four are Song rec +0x1E14, and on the mixed
  path `00 80 00 00`.

**Left to Logic.** LPFW calls these cosmetic, or recomputed by Logic on
save:

- the MSeq recency ordinal at rec +0x116;
- the hashes at rec +0x3C and +0x44 of the per-slot Trak rows;
- the third registry's time stamps;
- a MIDI region MSeq's id at rec +0x2C (body +0x08), which is *renumbered
  on save*. §2.6's counter at *MSeq +8* may be this field, if it counts
  from the body.

#### 9.6.5 Where LPFW disagrees with itself

- **Audio placement +0x2C.**
  - Spec §8.1: the region's index × 4, the same as its AuRg's and AuFl's
    index.
  - Spec §41.2 and `session.py`: the file's index × 4, with the region
    within the file at +0x28.

  The two agree when each file has one region, which is all the writer
  makes.
- **The Envi channel UUID.**
  - The writer puts it at name end + 0x11F.
  - Spec §19's survey of a corpus of saved projects puts it at name end +
    6 or 7 in body terms, and says +0x11F fits 4.7 % of records.
- **Moving a MIDI region.** `with_midi_region` adds 0x100 a bar to event
  +0x0C. `synthesize_midi_regions` adds it to event +0x30.
- **The MIDI region link.** Spec §8.5 takes the 0x58 shared by event
  +0x10 and the region's MSeq for the link. §41.1 puts it at event +0x20
  instead, and notes +0x08 is 0 on some regions (F4b). The writer sets
  +0x20. `session.py` reads +0x20 and falls back to +0x08.
- **The Song's registry.** Spec §8.1 finds it needed for the audio list.
  §10.7 finds that Logic rebuilds it on load. The code follows §10.7.
- **Header form.** Spec §2.1 says only `0x09cd` to `0x09cf` write form 2.
  GarageBand 10.4.14 writes form 2 with `0x09d0` (§2.5).

#### 9.6.6 What this means for GarageBand

What §11 to §13 found of each point:

- **The seeds are not GarageBand-shaped.**
  - A Logic seed: 440 AuCO, 151 Trak, 79 MSeq, 12 Envi, and a Song of
    26 392 bytes (`audio_base`).
  - GarageBand's *empty*: 283 AuCO, 28 Trak, 15 MSeq, 16 Envi, and a Song
    of 10 996 bytes (§2.5).

  The Song offsets LPFW uses (+0x80, +0xF4, +0x1E14, +0x1E20) were found
  on Logic's larger Song. In GarageBand's, +0x80 and +0xF4 hold the
  selected track (§11.3). GarageBand declines the seeds outright (§12.3)
  **[measured]**.
- **Trak size.** LPFW finds arrange rows by their size, 93 bytes, a
  57-byte body. On the mixed path it finds per-slot rows the same way.
  GarageBand 10.4.14 writes 58-byte Trak bodies (§2.5), so its code as
  written would find no rows in a `.band` (§11.5) **[measured]**.
- **The pre-allocated mixer is not needed.** LPFW gets it from Logic by
  adding 64 tracks and deleting 63, so that it never has to renumber the
  strips. §13 puts each new strip in its place and renumbers every later
  strip and the `Envi`s that name it, and GarageBand opens the result
  **[measured]**. Whether GarageBand keeps its strips after deleting tracks
  was not tried.
- **It was validated in Logic only.** Its author opened the output in
  Logic, and its tests compare bytes with Logic's saves. GarageBand opens
  neither LPFW's output nor Logic's own saves (§12) **[measured]**.
- **What is still of use.** Its encodings of notes, a MIDI region's start
  and its placement's event +0x0C, which §13 follows and GarageBand keeps
  (§13.2).
- **Licence.** The code is MIT, *Copyright (c) 2026 Jon Kubis*, and could
  come in with that notice. The seeds and `infra.json.gz` are bytes that
  Logic wrote (§7.2).

## 10. Projects one change apart, and a record diff

This section says how the projects were made, what the diff tool does, and
what each change adds, record by record. §11 says what the fields mean,
from these diffs and §9.6.

### 10.1 The fixtures

Saved by GarageBand 10.4.14 on 2026-11-03 **[measured]**. They are kept
outside the repository, since they are Apple's bytes (§7.2). A short
standard-library script, not committed, wrote their inputs:

- `one.mid`: format 1, 480 a quarter, 4/4, ♩ = 120. One track, *One*, with
  program 0 on channel 1 and C D E F as quarters in bar 1.
- `two.mid` adds a track *Two* (channel 2, G A B C), and `three.mid` a third
  track, *Three* (channel 3).
- `click.wav`: 2 s, 44.1 kHz, 16-bit stereo, a click every half second.

They were made in two ways:

- **By AppleScript alone:** `open` a MIDI file, `save document 1 in` a new
  path, `close saving no`. Each `open` makes a new document.
- **One document, changed at the screen.** The operator made one change at a
  time, and AppleScript saved after each one. `save in` writes a copy: the
  window stays *Untitled 10*.

| Fixture | How | Compared with | The change |
| --- | --- | --- | --- |
| `one-a` | `one.mid` opened and saved | | |
| `one-a2` | the same document saved again | `one-a` | none (a control) |
| `one-a-resaved` | `one-a.band` opened and saved | `one-a` | none (a control) |
| `one-b` | `one.mid` opened again | `one-a` | none, in a new document (a control) |
| `two`, `three` | `two.mid`, `three.mid` opened | `one-a`, `two` | one more MIDI track and its region |
| `c0-base` | `one.mid` opened and saved | | the chain starts |
| `c1-audio` | `click.wav` dragged below the tracks, at bar 1 | `c0-base` | an audio track, its region and its file |
| `c2-region` | `click.wav` dragged again onto that track, at bar 3 | `c1-audio` | a region and a file |
| `c3-copy` | the bar-1 region ⌥-dragged to bar 5 | `c2-region` | a region on the same file |

Two things showed up while making them:

- **A WAV dragged in twice is copied twice.** The second copy is
  `click_2.wav`, with its own `AuFl`. A region copied with ⌥-drag shares its
  file, as in `c3-copy`.
- **An undone drag leaves its copy behind.** The operator first dropped the
  second click at bar 1 and undid it. Its copy, `click_1.wav`, stays in
  `Media/Audio Files/`, and neither `ProjectData` nor `MetaData.plist` lists
  it.

Not made yet, for when the operator is back at the screen (§10.5): a split
MIDI region, an empty instrument track, and that track deleted again.

### 10.2 The tool

`spike/diff_band.py` compares two projects, or two bare `ProjectData` files,
record by record. It reads only, uses the standard library, and runs with
`python3 -I`:

```sh
python3 -I docs/research/garageband/spike/diff_band.py A.band B.band
python3 -I docs/research/garageband/spike/diff_band.py --ids A.band B.band    # the ids that changed, and where else B holds them
python3 -I docs/research/garageband/spike/diff_band.py --tag Trak A.band B.band
python3 -I docs/research/garageband/spike/diff_band.py --summary A.band B.band
```

- **Pairing by position.** The records' run of tag, class, index, length and
  name is aligned as a text diff aligns lines. What is left over is paired by
  tag, class and index. So an added record shows where it sits.
  `inspect_band.py --diff`, which pairs by tag, class and index alone, stays
  as it was.
- **Moves.** A record whose bytes are another record's is reported as
  *moved*, or as *re-sorted* when two records at one index swap places. A
  keyed diff would show a re-sort as two changed bodies (§9.6.4's ranks).
- **Bytes.** Each changed pair lists its differing runs at *rec +* and
  *body +* (§9.6). An `EvSq` is compared by 16-byte rows, so an added event
  shows as rows.
- **Noise, folded** (§10.3): ids, the Song's rows of stamps, and runs of
  records whose small numbers changed alike, such as 254 `AuCO` renumbered
  after a new strip.

**Checked [measured]:**

- A project against itself: every record is the same.
- §5's patched variants against their template give what §5.2 lists, and
  nothing else:
  - `rename`: the three region names and the two strip names, with their
    lengths;
  - `tempo`: the two tempo events, the Song's +110 and +114 (rec +0x92 and
    +0x96), and `BeatsPerMinute`;
  - `media`: the `AuFl`, the `AuRg` and the strip's `Envi`.
- Copies of `c3-copy`'s `ProjectData` with their records reordered:
  - two `Trak` rows at index 2 swapped show as *re-sorted*;
  - an `Envi` moved five places shows as *moved*;
  - a duplicated `MSeq` and a dropped `AuCU` show as one added and one
    removed.

### 10.3 What the controls show

All **[measured]**, on the fixtures of §10.1:

- **Saved twice.** One u16 changes: the Song's body +0x86, from 1 to 8409.
  8409 is `projectAssetFlags` (§2.3), which both saves have in
  `ProjectInformation.plist`.
- **Reopened and saved.** Every sequence's counter, §2.6's *MSeq +8*
  (body +0x08), takes a new value. So does the byte at rec +0x0E of the
  `EvSq` after it. In every fixture, in the Voice template and in §4's
  `stems.band`, the two bytes are equal: 15 of 15 pairs in `one-a`, 40 of 40
  in `stems`. The arrange's `MSeq` takes the package's name (*one-a*). Seven
  ids in the Song and a few of its bytes change, and so do two `AuCO`.
- **The same MIDI file opened twice.** 194 ids in 57 records are new, 138 of
  them in the Song. An id is 16 bytes. Bytes 4 to 7 stay the same, and bytes
  6 and 7 are `11 f1` in every id seen. That is the mark of a version-1,
  time-based UUID, and it matches LPFW's *4 random, 4 fixed, 8 random*
  (§9.6.4). Bytes 4 and 5 went from `d7 84` to `d7 89` during the session.
  The Song also has rows of u32s, 16 bytes apart, that change with each open
  (LPFW's stamps, §9.6.4).
- **Inside one document, no id changes.** `c0-base` against `c1-audio` has
  none. So the chain's diffs show only what the operator changed, and
  diffs between separate opens need the folding.

### 10.4 What one change adds

**[measured]**, records only:

| Change | Records added | What else changes |
| --- | --- | --- |
| **A MIDI track and its region** (`one-a` → `two`, `two` → `three`, the same both times) | one `AuCO` and ten `AuCU` of the sizes of the first strip's; one `Envi` at the next index (42, then 43), named after its patch; one `Trak` at index 1 and one at index 2; three `MSeq`–`Trak`–`EvSq` triples, each with an empty `Trak`: the region (*Two*, its notes in a 160-byte `EvSq`) and two *\*Automation* | each later `AuCO` gets +1 at rec +0x0E; the Song grows by 160 bytes; `NumberOfTracks` |
| **An audio track, its region and its file** (`c0-base` → `c1-audio`) | `AuFl` and `AuRg` at index 0; three `Envi`: the strip *click* (42) and two send strips, *Ambience/0.2s Long Ambience* and *Small Hall/1.6s Short Vocal Hall*; one `AuCO` and 23 `AuCU`; one `Trak` at index 1 and three at index 2; three *\*Automation* triples; a `GenM` | each later `AuCO` gets +1 at rec +0x0E and +2 at body +0x1C; the Song grows by 320 bytes; `ImpulsResponsesFiles` gains the hall |
| **A region and a file** (`c1-audio` → `c2-region`) | `AuFl` and `AuRg` at index 1; a `GenM` | an 80-byte `0x24` event in the arrange's `EvSq`; the Song grows by 80 bytes; four `AuCU` of 7.5 to 8.7 KB, plug-in state of the strips, change in hundreds of places, which they do not between `c2-region` and `c3-copy` |
| **A region on the same file** (`c2-region` → `c3-copy`) | an `AuRg`, *click.1*, at index 0 after the file's first region, with 1 at rec +0x0E; a `GenM` | an 80-byte `0x24` event at bar 5; the Song grows by 40 bytes |

- **Sequence indexes are not appended.** In class 23, the region *Two* takes
  index 8, which in `one-a` was an *\*Automation* sequence. *Three* takes 10.
  A MIDI track's region sits after the automation sequence of the track
  before it, and the other automation sequences come after the regions. A
  diff by index therefore pairs *Two* with `one-a`'s old index 8. The
  records added are the three triples at the end.
- **The first audio track brings the reverb sends.** Their two strips, and
  the hall in `MetaData.plist`, arrive with it.
- **The Song grows in rows** of 24 bytes (`[u32 tag][u32][16-byte id]`, as
  in LPFW's registry, §9.6.4) and 16 bytes. They are the two tables of the
  `Song`'s registry (§13.2).

### 10.5 What is still to make

Three fixtures need the operator at the screen, each saved by AppleScript
after the change, in the chain's document (or in `c3-copy.band` opened
again):

- select the region *One*, put the playhead at bar 1 beat 3, and choose
  Edit › Split Regions at Playhead (⌘T): one MIDI region becomes two;
- Track › New Track…, Software Instrument, Create: an empty instrument
  track;
- delete that track again: does GarageBand keep its strip, as Logic keeps
  its pre-allocated mixer (§9.6.6)?

None of them was needed for §11 or §13.

## 11. How the records link

These links were decoded from §10's diffs and §9.6's model, then checked
on every project at hand. All of it is **[measured]**, on 31 projects:

- the 15 that GarageBand 10.4.14 saved from scratch: §10.1's ten
  fixtures, §2.1's four projects, and §4's `stems`;
- the six templates, saved by 10.2.0;
- the ten projects of §5 that GarageBand opened and saved again.

`inspect_band.py --links` prints each project's tracks, strips, regions and
files, and checks every link below. It exits 1 if one fails:

```sh
python3 -I docs/research/garageband/spike/inspect_band.py --links PROJECT.band
```

Every check holds on 29 of the 31. The other two are §5's `rename` and
`all` after GarageBand saved them, and §11.4 says why.

Offsets are counted as in §9.6: *rec +*, *body +* (rec + 0x24) and
*event +*. An *index* is the u32 at rec +0x08. A *slot* is the index
shifted right by 16, so `Envi` 0xa4 is index 0xa40000. §2.5's index is the
index shifted right by 18.

### 11.1 Objects

- **A record's class and index name an object** (rec +0x06 and rec +0x08).
  The records of one object sit together. Three kinds have more than one:
  - **A sequence** is an `MSeq`, then its `Trak` rows, then an empty `Trak`
    with 0x7fffffff at rec +0x12 that ends them, then its `EvSq`. The
    `MSeq` and the `EvSq` share the class and index. The `Trak`s have class
    23 and the same index. A row's place, from 0, is at rec +0x12.
  - **An audio file** is its `AuFl` and then its regions' `AuRg`, at one
    index. A region's number within the file, from 0, is at rec +0x0E.
  - **The channel strips** are `AuCn` 9 (class 14), then each strip's
    `AuCO` and its plug-ins' `AuCU`. An `AuCO`'s number is at rec +0x0E,
    and an `AuCU` carries the number of the `AuCO` before it. `AuCn` 8,
    10 to 17 and 30 to 32 hold one `AuCO` each.
- **References are by index, slot or number, never by offset.** No record
  holds another's position in the file.
- **The sequences of class 23** are the song's. Index 0 is *TRASH*, 1 the
  arrange and 2 the automation folder (*Track Automation Root Folder*, or
  *Automation* in the templates). *Track Alternatives*, *Global
  Harmonies* (10.4.14 only), and one sequence per MIDI region and per
  automation lane come after them. GarageBand numbers them again on save,
  so a region's index can change (§10.4).

### 11.2 The links

| From | Field | To |
| --- | --- | --- |
| A track row (a `Trak` of the arrange) | body +0x06, u32 (rec +0x2A) | its channel strip's `Envi`, by index. body +0x08 is the slot alone |
| A track row | body +0x00, u16 | the row's kind: 1 a track, 3 the master, which is always last, 4 a Drummer track (in the templates) |
| A track row | body +0x02, u16 | 0x14 set when the row has an event in its folder's `EvSq`, in every row of both folders |
| A placement (an 80-byte `0x20` or `0x24` event in a folder's `EvSq`) | event +0x14, u16 | its row, counted from 1 in file order. Since the master is last, a track's number is its place |
| A placement | event +0x10, u16 | its row's slot, again |
| A `0x20` placement | event +0x20, u32 | the region's `MSeq` of class 23, by slot: 0x18 is index 6. event +0x08 held another value in 473 of the 474 `0x20` placements, so it is not this link |
| A `0x24` placement | event +0x2C and +0x28, u32s | the `AuFl`, by slot (file *i* is 4 *i*), and the region's number within that file |
| A region's `MSeq` | u32 at 0xCA after its name (§11.4) | the `Envi` of the track it is on, by index |
| An `Envi` | u16 just after its name | its `AuCO`'s number, plus one |
| An `Envi` | 16 bytes at 0x11F after its name | the 16 bytes after a 0xff in that `AuCO`, at rec +0xBD to +0xFD. They are an id for the strips, and other bytes for the buses. Checked in the 15 projects 10.4.14 saved; the templates' `Envi` has another layout there |
| An automation row (a `Trak` of the automation folder) | body +0x06 | an `Envi`, by index. A channel whose automation lane exists has a row and an event pointing at its *\*Automation* `MSeq`. The templates keep rows for 1 to 9 strips that are not in the file, and GarageBand offers them all the same |
| An `AuRg` | its index | the `AuFl` of the same index |

The ids in the rows (body +0x18) and in the `AuRg` records appear in no
other record. In 10.4.14 they are version-1 UUIDs (§10.3). In the templates
they are random, version 4.

### 11.3 Counters and bookkeeping

- **The sequence counter**, §2.6's *MSeq +8*, is a u32 at the `MSeq`'s
  body +0x08. Its `EvSq` repeats it at rec +0x0E. The counters are
  distinct within a project and small (0 to 90 seen). GarageBand renews
  them on every save (§10.3). 10.2.0 wrote them nearly in file order from
  0, and 10.4.14 in no order found. What they count is open.
- **The arrange's height**, the u16 at 0x0A after its name, is 0x3C times
  its rows, the master included: 120 for one track, 720 for eleven. This is
  LPFW's rule, and it held in all 31.
- **`NumberOfTracks`** in `MetaData.plist` is the arrange's rows less the
  master, in all 31.
- **The `Song` holds the selected track, not a count.** Its u16 at rec
  +0x82 is the slot of the row whose body +0x28 has 0x20 set. Its u16 at
  rec +0xF6 is that row's number. LPFW reads rec +0x80 as the highest
  channel and rec +0xF4 as the track count. They hold those values just
  after a track is made, because a new track is selected. A MIDI file opened
  in GarageBand leaves no row selected and all three fields 0, even with
  three tracks (`three`). In the templates the selected track is the first
  or second of 5 to 9.
- **The `Song`'s registry.** A 10.4.14 `Song` has a 24-byte row,
  `[u32 class][u32 slot][16-byte id]`, for each `Envi` (class 20, 0x14) and
  each sequence of class 23 (0x17). Each table holds exactly the objects in
  the file: 16 and 9 rows in `empty`, 43 and 34 in `stems`. Its ids appear
  in no other record. The templates' 3.7 KB `Song` has no registry, and
  GarageBand opens them. Whether it rebuilds a missing registry in a
  `0x09d0` file was not tried. LPFW's spec §10.7 finds that Logic does.
  §13.2 found that these rows are one table of objects of every class from
  1 to 33, beside a second table of time stamps.
- **Strip numbers** count the `AuCO`s in file order. A strip put in
  renumbers every later `AuCO` and its `AuCU`s, and the `Envi`s that name
  them (§10.4: +1 at rec +0x0E, and +1 just after a strip's name).
- **A new channel** takes the slot after the highest in use: 0xa4, 0xa8
  and 0xac for `one-a`, `two` and `three`. The first audio track takes 0xa8
  in `c1-audio`, and its two reverb sends 0xac and 0xb0.

### 11.4 Names end at an even offset

- **In every `MSeq`, `Envi` and `AuRg` GarageBand wrote**, the fields
  after a name start at the name's end, rounded up to an even offset. *One*
  ends at rec +0x39, and its fields start at +0x3A. *Drum kit* ends at
  +0x3E, and its fields start there. §11.2's offsets after a name count
  from that even offset.
- **§5's patcher ignored this.** `patch_band_template.py` swaps a name
  without adding or dropping the pad byte. Two of its renames changed a
  name's parity:
  - *Drums* to *Drum kit* (odd to even) left a spare byte. GarageBand
    saved the record framed right, one byte shorter.
  - *Violin* to *Violin I (solo)* (even to odd) put the fields a byte early.
    GarageBand opened the project and played it (§5.3). It saved a record
    whose bytes after the name are partly shifted by one and partly doubled
    (`b0 b0`, `fc fc`, `04 04`). The region's channel came back as
    0x00b0b000, which names no `Envi`. Nothing said so.
- So a writer must pad names to even. After §5.4's `bad-name`, this is
  the second field that GarageBand read wrong and saved without a sign.

### 11.5 LPFW's model (§9.6) against GarageBand's files

| LPFW (§9.6) | In GarageBand's files |
| --- | --- |
| The arrange's `MSeq`, `EvSq` and rows at index 0x040000 | holds: class 23, index 1 |
| Per-slot rows at 0x080000, re-sorted by rank | these are the automation folder's rows. A row's rec +0x12 is its place, and nothing else was seen to rank them |
| A track row is 93 bytes, with a 57-byte body | 57 in the 10.2.0 templates, as in Logic's seeds. 10.4.14 writes 58. LPFW's search by size would find no rows in a 10.4.14 project (§9.6.6) |
| Channel at rec +0x2A, slot at +0x2C, id at +0x3C, 0x20 at +0x4C on the newest row | holds. The 0x20 marks the selected row |
| A region's channel at 0xCA after its `MSeq`'s name | holds, counted from the even offset |
| A region's length at rec +0x78 and start at rec +0x11C | the length holds: 0x3C after the name, 3840 for one bar. The start is untested, since every region here starts at bar 1 |
| `0x20` event +0x20 is the region's index >> 16 | holds. event +0x08 is not the link (§9.6.5) |
| `0x24` event +0x2C is the file's index × 4, +0x28 the region within it (spec §41.2) | holds. In `c3-copy` the ⌥-dragged copy has 1 at +0x28 and 0 at +0x2C, and `click_2.wav`'s region has 4 at +0x2C. Spec §8.1's *region × 4* is wrong once a file has two regions |
| event +0x14 is the track's place among the arrange's rows, the master left out | holds |
| The `Envi`'s channel UUID at 0x11F after its name equals `AuCO` rec +0xBD | holds, counted from the even offset. In the `AuCO` it follows a 0xff whose place varies with the strip |
| `Song` rec +0x80 is the highest channel, and +0xF4 and +0xF8 count tracks | they hold the selected track (§11.3) |
| The arrange's height; `NumberOfTracks` | hold |
| Registry rows `[0x14][slot][UUID]` | hold, beside a second table, `[0x17][slot][UUID]`, of class-23 sequences |
| The bumps at `Envi` rec +0xCA, +0xCC and +0xCF | the strip number just after the name, at the offset LPFW's names give it. Its other `Envi` bumps were not checked |
| `GenM`'s `drummerModelTrackStates` is keyed by track id | `{}` in every 10.4.14 project here, since none has a Drummer track. Each audio region adds a `GenM` of loop metadata (§10.4) |
| A region `MSeq`'s id is renumbered on save | holds: it is the sequence counter |

### 11.6 What is still open

- **The sequence counter's rule.** It is not needed. §13 gives each new
  sequence the next value above the highest in the project, and
  GarageBand opens the project and renews every counter on save
  **[measured]**. What GarageBand's own values count is still unknown.
- **Two fields of every placement.** Event +0x0C is 0x400 on MIDI regions
  at bar 1 and grows by 0x100 a bar (§13.2). Its top bit, set on the
  ⌥-dragged copy (0x80000100), is not decoded. Event +0x30 is 0x600 on
  every placement seen.
- **What GarageBand needs to open a project.** It needed neither a right
  channel in a region's `MSeq` (§11.4) nor a selected track (the MIDI
  imports). §13 writes the registry, the counters and the automation rows
  and lanes as GarageBand does, and GarageBand opens it. Whether it would
  open a project without them was not tried.
- **The plug-in state** in `AuCO` and `AuCU`, so a new strip is still a
  copy of one that GarageBand wrote, with the same instrument.
- **The rest of the `Song`.** Its registry, with the time stamps that
  change on each open (§10.3), is decoded (§13.2). Most of its other bytes
  are not, and §13 copies them.
- **A Drummer track's links** through `GenM`. A region that starts after
  bar 1 is answered (§13.2).
- **The fixtures still to make** at the screen (§10.5): a split region, an
  empty instrument track, and that track deleted.

## 12. LogicProFormatWriter's output in a `.band`

LogicProFormatWriter (LPFW, §9.6) was run on §4's MIDI and stems. Each
`ProjectData` it wrote went into a `.band`, and GarageBand was asked to
open it. **GarageBand 10.4.14 opened none of them.** It did not open
Logic's own saves either, while a `ProjectData` that GarageBand wrote
opened in the same package. All of it is **[measured]**, on 2026-11-03.

### 12.1 The spike

```sh
python3 -I docs/research/garageband/spike/make_import_bundle.py OUT --lpfw LPFW_CLONE
python3 -I docs/research/garageband/spike/wrap_projectdata.py SOURCE.logicx OUT.band
python3 -I docs/research/garageband/spike/wrap_projectdata.py --open DIR [NAME ...]
```

- **LPFW** was cloned at commit `9935ca8` into its own scratch folder. Its
  code was read before it was run, under `python3 -I`. It reads only its
  seeds and the inputs, and writes only the output it is given.
  `make_import_bundle.py --lpfw` imports it from the clone and copies none
  of it. It makes three projects through LPFW's own calls:
  - `seed`: `synthesize_av_region_bundle` with nothing added, so LPFW's
    mixed seed as Logic 11.2.2 saved it, with one instrument and one audio
    track;
  - `audio`: `export_beatmap` of `Lean.mid` with the three stems at tick 0,
    from the 64-strip audio seed. That gives three audio tracks, the tempo
    map and the meter;
  - `av`: the mixed seed with five instrument tracks and two audio tracks
    added, so one track per part and one per stem. Each part has a MIDI
    region with its notes at 960 a quarter, each stem has an audio region
    at tick 0, and the tempo map and meter are set.
- **Two things the driver had to work around:**
  - the mixed seed already has *Inst 1* and *Audio 1*, so one fewer of
    each is added, and those two are renamed;
  - LPFW writes names as Latin-1 (`_set_ivne_name`, `_set_region_name`),
    and GarageBand writes UTF-8 (§2.6). So *Piano · G clef* is passed to it
    as its UTF-8 bytes.
- **`wrap_projectdata.py`** builds the package of §2.2. It copies GarageBand
  for iOS's stub `projectData` from a template in GarageBand.app (its MD5 is
  checked against §2.4's). It takes `ProjectData`, `MetaData.plist` and
  `Media/Audio Files/` from the source. It writes
  `Resources/ProjectInformation.plist` as 10.4.14 does, or keeps the
  source's with `--logic-info`. There are no `DisplayState` files and no
  `WindowImage.jpg`.
- **`--open`** is §5.1's loop: `open`, `save document 1 in` a new path,
  `close saving no`, then the read-back and `inspect_band.py --links`. It
  saves only a document whose `path` is the project it opened. A project
  that opened late was once taken for the next one. It also reports any new
  `GarageBand*` file in `~/Library/Logs/DiagnosticReports/`.

### 12.2 What LPFW wrote

`inspect_band.py --links` on the three, before GarageBand saw them:

- **`seed`**, which is Logic's bytes, passes every check of §11.2. The
  only notes are on automation rows whose strips are not in the file.
- **`audio` fails 5.** Its three new rows lack the 0x14 flag, and two
  placements carry the prototype's slot at event +0x10 (LPFW keeps 0x58
  there, §9.6.3).
- **`av` fails 35:**
  - the six MIDI regions share one sequence counter, 98;
  - the `Song`'s registry lists 13 of the 20 `Envi` and 71 of the 77
    sequences;
  - the rows' 0x14 flag and the placements' slot at event +0x10 do not
    match their rows;
  - the five added instruments name one strip number (102), and the two
    added audio tracks another (1).

  LPFW leaves the counters and the registry to Logic (§9.6.4).
- **The seeds are Logic 11.2.2's format** (`0x09cf`). Their record schema
  versions are older than 10.4.14's: `AuCO` 6 against 7, `AuCU` 4 against
  5, `Trak` 5 against 6. They also hold a `TxSq` record, and `audio` a
  `Trns`, which no GarageBand project of §11 has. Their `Song` is 16 or
  26 KB, against 11 KB in 10.4.14's projects.
- **Logic's own saves look like GarageBand's** by these measures. A project
  that Logic 12.2 saved (`0x09d0`) has 10.4.14's schema versions
  throughout, no `TxSq` or `Trns`, and a `Song` of 10 876 bytes. Logic
  11.2.2's `F0_baseline` has a `Song` of 10 756.

### 12.3 What GarageBand did

GarageBand was quit and launched again before the run. Each project was
opened alone, with no other document open.

| Project | What it is | Format | `open` returned | What happened |
| --- | --- | --- | --- | --- |
| `rewrap` (control) | §2.1's `audio`, which GarageBand saved: its `ProjectData`, `MetaData.plist` and WAV in a package made by `wrap_projectdata.py` | `0x09d0` | the document | **opened**, not marked modified, with no dialog. Saved again, it kept its track, its region at bar 2 and its file, and every check of §11 held. GarageBand added an `Undo Data.nosync` folder to the package it opened |
| `seed` | LPFW's mixed seed, nothing added | `0x09cf` | `missing value` | **declined**: no document and no alert. The same twice, once after a fresh launch |
| `av` | LPFW: 9 tracks, notes, stems, tempo map | `0x09cf` | `missing value` | **declined**, the same way |
| `audio` | LPFW's beatmap: 3 audio tracks | `0x09cf` | nothing, after 180 s | **an alert** |
| `logic122` | LPFW's fixture `TAKES`, saved by Logic Pro 12.2 (6644) | `0x09d0` | nothing, after 180 s | **an alert** |
| `logic1122` | LPFW's fixture `F0_baseline`, saved by Logic Pro 11.2.2 (6387) | `0x09cf` | nothing, after 180 s | **an alert** |

- **The alert** reads *The operation couldn't be completed. (OSStatus
  error -19.)*, with one button, *OK*. The operator saw it during the three
  hung opens, one at a time. −19 is `readErr` in macOS's headers, an I/O
  error. AppleScript cannot see it: `name of every window` lists only the
  project chooser, and `open` waits behind it until the AppleEvent times
  out.
- **No crash.** No crash report was written for any of the six.
- **Not run:** the seed with its format code set to `0x09d0`
  (`seed-d0`), and the seed with Logic's `ProjectInformation.plist`
  (`seed-logicinfo`). The run was stopped at the operator's request. An
  earlier run of them came after a hidden alert and does not count.
  `logic122` already has GarageBand's format code and is refused, so the
  code alone is not what GarageBand checks.

### 12.4 What it means

- **The package is not the problem.** A `ProjectData` that GarageBand wrote
  opens in a package made from nothing but the stub, `MetaData.plist`,
  `ProjectInformation.plist` and `Media/`.
- **GarageBand refuses `ProjectData` that Logic wrote**, LPFW's or Logic's
  own. That holds at Logic 12.2's format code and schema versions, and
  whatever `ProjectInformation.plist` says. GarageBand checks something
  inside the song that this step did not find. It is not the schema
  versions, the `Song`'s size, or the `TxSq` and `Trns` records, since
  Logic 12.2's save matches 10.4.14 in all three and is refused. Nor is it
  §11's links, which hold in `seed`. A record diff of `logic122` against a
  10.4.14 project (`diff_band.py`) is the next place to look.
- **It refuses in two ways.** The two projects from LPFW's mixed seed were
  declined at once and without a word. Logic's saves and the beatmap, from
  the 64-strip audio seed, stopped at an alert. Neither crashed GarageBand,
  unlike §5.4's mis-framed record.
- **So LPFW's seeds cannot be GarageBand's starting point.** A writer for
  GarageBand would start from a project GarageBand saved, as §5 does, and
  build from §11's decoded links. §13's builder does that. LPFW's code
  remains a reference for the encodings of notes, regions and audio files
  (MIT, §9.6.6). Its seeds and Logic's fixtures do not help.
- **For an export, the alert matters.** A Logic-made project handed to
  GarageBand would stop at a modal alert that a script cannot see or
  dismiss.

## 13. A project of a new shape, built from one GarageBand saved

`spike/build_band.py` writes projects whose shape no template has: up to
14 tracks, 17 MIDI regions and 7 audio files, against the templates' 5 to
9 tracks (§2.1). The base was §10.1's `c3-copy`, which has 2 tracks.
**GarageBand 10.4.14 opened all of them**, with no dialog and no crash. It
saved them again with every record and every link of §11 intact, and the
operator saw and heard the 14 tracks. All of it is **[measured]**, on 2026-11-03.

### 13.1 The spike

```sh
python3 -I docs/research/garageband/spike/build_band.py BASE.band OUT_DIR [VARIANT ...]
python3 -I docs/research/garageband/spike/build_band.py --open OUT_DIR [VARIANT ...]
```

- **The base** is a project GarageBand saved, with a MIDI track holding a
  region and an audio track holding a region. Its own tracks are the
  exemplars, and every record added is a copy of one of the base's.
  `c3-copy` has *One* (Steinway Grand Piano) and *click*, three audio
  regions and two files. It stays outside the repository (§10.1).
- **The variants:**

  | Variant | What it adds to the base | Tracks |
  | --- | --- | --- |
  | `copy` | nothing: the records split and joined again (a control) | 2 |
  | `regions` | a MIDI region at bar 5 on *One*; two new audio files on *click* at bars 7 and 9, and a second region of the first at bar 11 | 2 |
  | `midi1` | a MIDI track with a region of four notes | 3 |
  | `audio1` | an audio track with a new file | 3 |
  | `big` | 8 MIDI tracks with regions at bars 1 and 5, and 4 audio tracks, each with a new file at bar 1 and a second region of it at bar 5; the last also has a second file at bar 9 | 14 |

- **What a track brings.** These are the records GarageBand adds for a
  track (§10.4), with the links of §11.2 set:
  - an `Envi` at the next free slot, named after the track;
  - a strip (`AuCO` and its `AuCU`s), put in its place among the strips;
  - an arrange row before the master;
  - an automation row, its *\*Automation* sequence and its event;
  - one sequence and one placement per MIDI region;
  - an `AuFl` per file, and an `AuRg` and a `GenM` per audio region;
  - a row in both of the `Song`'s tables for every new object (§13.2).

  Sequence counters are new and distinct. Names are padded to even
  (§11.4). The ids are fresh version-1 UUIDs. Within one track, the same
  old id becomes the same new one, so a strip's id stays equal in its
  `Envi` and its `AuCO`.
- **`--open`** is §12.1's loop: `open`, `save document 1 in`, `close
  saving no`, then the read-back and `inspect_band.py --links`.
- **Licence.** `build_band.py` is our own code, standard library only. It
  copies nothing from LPFW or any GPL project. It follows two of LPFW's
  findings (§9.6.2, §9.6.5):
  - a MIDI region's start, at 0xE0 after its `MSeq`'s name;
  - a placement's event +0x0C, which grows by 0x100 a bar.

  It reuses `inspect_band.py` and `patch_band_template.py`.

### 13.2 What building it decoded

- **The `Song`'s registry is two object tables.** It is not only §11.3's
  `Envi` and class-23 rows:
  - The first table has 24-byte rows `[u32 class][u32 slot][16-byte id]`.
    The second has 16-byte rows `[u32 class][u32 slot][u64 time]`, where
    the time is the 60-bit timestamp of a version-1 UUID.
  - Both list every object of classes 1 to 33, sorted by class and then
    slot. That includes `AuFl` (class 11), `Envi` (20), the sequences (22,
    23, 25) and `GenM` (30). `AuCO` and `AuRg` have no rows.
  - Each table ends with a row `23 00 00 00 ff ff ff ff`. Neither has a
    count. A new object takes a row in each, in sorted place.
- **The strips.**
  - `AuCn` 9 counts them by kind: all at body +0x1A, audio at +0x1C and
    instruments at +0x22. Its body ends in a run of u32 1s, one per strip.
  - An `AuCO`'s body +0x04 is its kind: 0x40 audio, 0x43 instrument, 0x45
    bus. Its body +0x06 is its place within its kind. Its own name
    (*Inst 3*, *Audio 2*) is 16 bytes at rec +0x61.
  - A new strip goes just after the last strip of its kind that a track
    uses. Spares come after it, and `Preview` uses one of them, which is
    why "the last strip named by any `Envi`" is the wrong rule. Every
    later strip is renumbered, and so are the `Envi`s that name them.
- **The `Envi` of a track.** It names itself at rec +0x08 and again at rec
  +0x34. The u16 at rec +0x76 grows by 0x42 per channel (LPFW's x
  position). rec +0x51 is 1 on the newest track's channel and 3 on the
  others.
- **An automation lane's sequence** names its channel at 0xCA after its
  name, as a region's does. `inspect_band.py --links` caught a copy that
  still named the exemplar's.
- **A MIDI region after bar 1** has its start, zero-based, at 0xE0 after
  its `MSeq`'s name. Its placement's event +0x0C is the exemplar's plus
  0x100 a bar. Its notes count from the region's start, at 38400.
  GarageBand kept all three as written, so §11.6's question about such a
  region is answered.

### 13.3 What GarageBand did

GarageBand had no project open, and each project was opened alone.

| Variant | `open` | Marked modified | Saved again | §11's checks on the save |
| --- | --- | --- | --- | --- |
| `copy` | the document, no dialog | no | yes | all held |
| `regions` | the document, no dialog | yes | yes: *One again* at bar 5, *Tone 1* and *Tone 2* at bars 7 and 9, *Tone 1.1* at bar 11 | all held |
| `midi1` | the document, no dialog | no | yes: *Part 2* on strip *Inst 2* | all held |
| `audio1` | the document, no dialog | yes | yes: *Stem 2* on strip *Audio 2* | all held |
| `big` | the document, no dialog | yes | yes: 14 tracks, 17 MIDI regions, 7 files, 811 records, the same number as written | all held |

- **No crash report** was written for any of the five.
- **Marked modified** means GarageBand wrote waveform data into new WAVs
  (§5.3). The `LGWV` chunk was missing from the WAVs this spike wrote.
  `midi1` adds no audio, and it opened unmodified.
- **The samples** of every WAV in `big`'s save are the ones written. The
  notes of *Part 2 B* and *Part 9 B* read back as written.

**What GarageBand changed when it saved `big`.** Record counts and tags
were the same as written. The changes:

- every sequence's counter, and the regions' recency bytes (§9.6.4),
  which it renews on every save (§10.3);
- in the `AuFl`s, each new file's `LGWV` size and a few other bytes;
- in each copied audio strip, the third and fourth 76-byte `AuCU`s: the id
  at body +0x3C cleared and body +0x14 set to `ffff`. The exemplar's
  strip, *Audio 1*, keeps them, so these are likely links that GarageBand
  drops when they are not unique. The audio strips' byte at rec +0x70 went
  from 0 to 0xab, the exemplar's too, and two spare audio strips grew;
- the order of the placements in the arrange's `EvSq`;
- the `Song`'s stamps and a few of its bytes. All the registry rows the
  builder added were kept;
- `MetaData.plist` rewritten with absolute paths, and `click_1.wav`, the
  base's unused copy (§10.1), dropped from `Media/`.

### 13.4 By eye

The operator opened `big.band`, the builder's output and not GarageBand's
save, and checked it against these steps **[measured]**:

1. The track headers read, from the top: *Steinway Grand Piano*, *click*,
   *Part 2* to *Part 9*, *Stem 2* to *Stem 5*.
2. Each *Part* track has two green MIDI regions, at bars 1 and 5.
3. Each *Stem* track has blue audio regions at bars 1 and 5, and *Stem 5*
   a third at bar 9.
4. Space from bar 1: the parts play piano notes, and the stems play beeps.
5. Close without saving.

All of it was as described, with no dialog. AppleScript then closed the
project without saving.

### 13.5 What it means

- **A native writer can change a project's shape.** Starting from a
  project GarageBand saved, and copying its tracks with §11's links and
  §13.2's bookkeeping, gives projects that GarageBand opens, plays and
  saves without loss. §5.5's "as a general `.band` writer: no" rested on
  the links being opaque, and they are not.
- **What it does not show:**
  - *Which of the bookkeeping is needed.* The builder writes the registry
    rows, the counters and the automation lanes, as GarageBand does. Whether
    GarageBand would accept a project without them was not tried, so that
    part of §11.6 stays open as "what is needed", not "what works".
  - *Another instrument.* Every new MIDI track has the exemplar's patch
    (a Steinway), because its plug-in state is copied. Choosing a sound
    means a base with that patch, or decoding `AuCO`/`AuCU` (§11.6).
  - *Another base or version.* Only `c3-copy`, saved by 10.4.14, was used.
    A base needs a MIDI track and an audio track, each with a region. Tempo
    and meter were left as they were; §5 changes those.
- **The ids' clock.** The builder's UUIDs carry the time they were made, as
  GarageBand's do, so each run writes other ids. Rebuilt, the five projects
  differ from the tested ones in ids and nothing else.

The base and the outputs are Apple's bytes and stay outside the
repository. `build_band.py` writes the outputs again from the base.

## Appendix A. Reading lilypond's MIDI

The score of §1.2, compiled with `lilypond -s score.ly`:

```lilypond
\version "2.24.0"
\header { title = "Test" }
\score {
  <<
    \new Staff = "melody" \with { instrumentName = "Flute" midiInstrument = "flute" }
      \relative c'' { \tempo 4 = 100 \key g \major \time 3/4 \partial 4 d4 | g2 a4 | \tempo 4 = 80 b2. }
    \new PianoStaff \with { instrumentName = "Piano" } <<
      \new Staff = "rh" { \clef treble <g' b'>2. | <a' c''>2. }
      \new Staff = "lh" { \clef bass g,2. | d2. }
    >>
    \new DrumStaff \drummode { bd4 sn sn | bd4 sn sn }
  >>
  \layout { }
  \midi { }
}
```

The words were checked with a second score,
`\new Staff \with { midiInstrument = "choir aahs" } \new Voice = "v" { c'4 d' e' f' } \addlyrics { la la la la }`.
A short Python script, not committed, walked each `MTrk` chunk. It printed
every meta event with its tick, every program change and controller, and
the channels of the note-ons.
