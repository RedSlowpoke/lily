# DAWs for arranging a score from Lily Studio

Which digital audio workstations (DAWs) a Lily Studio user can take a score
to, to make a new arrangement of a song: which are free, which take the
studio's files best, and what the studio could write so that a score opens in
one as a ready project. The note then asks whether the studio should have a
small DAW of its own. It starts from what the studio already hands over (§1).
It then surveys the DAWs (§2), picks the best for each platform (§3), ranks
the files the studio could write (§4), weighs a DAW inside the studio (§5),
and ends with a recommendation and a staged plan (§6, §7).

Nothing here is decided. A decision in `docs/DECISIONS.md` should follow if
the work goes ahead. **[measured]** marks the one thing checked on this
machine (§5.3). Every other claim comes from the vendors' pages, manuals,
repositories and forum reports linked beside it, as read on 2026-11-04.
**[unverified]** marks what no source settled. Prices are list prices in the
vendor's currency and change often.

## 1. What the studio hands over today

- **Export MIDI** (D54): lilypond's MIDI file (format 1, 384 ticks a
  quarter), without the muted parts. lilypond names each track
  `<staff id>:<voice id>`, writes no `instrumentName`, and writes a program
  change only where the score sets `midiInstrument`. The words are a track
  of lyric events of their own (`docs/research/garageband/README.md` §1.2).
- **Export for GarageBand** (D162 to D165) writes `<score> (GarageBand)/`
  in the Mac app:
  - the MIDI **rewritten**: each part is named for its part, on the Parts
    fold's program, at `% BPM:`, with a lead-in after a pickup; the words'
    tracks and silent staves are dropped (`garageBandPlan.ts`,
    `crates/session/src/midi_export.rs`);
  - the recording and the stems as **WAVs aligned to the MIDI's tick 0**
    (`wav_align.rs`);
  - `Read me.txt`.

  The web build downloads the `.mid` alone.
- **Playback** (D24, D35, D45, D121, D155): `packages/common/web/midi.js`, a
  Web Audio synthesizer of two oscillators and an envelope per note, one voice
  per General MIDI family, with a volume and a mute per part and an audio track
  beside it. D24 chose it over a SoundFont, which then meant 10–150 MB and a
  licence. It is for proof-listening, not for an arrangement anyone would
  keep.

So the studio already makes the two things every DAW takes: a multi-track MIDI
file and WAVs that start where it starts. Only the GarageBand export makes
them carefully, and most of that care is not specific to GarageBand.

## 2. The DAWs

### 2.1 In short

*Free* below means usable for good at no cost. A trial that stops saving is
marked as such.

| DAW | Cost (2026-11) | Platforms | Imports a format-1 MIDI file | MusicXML | Project file | Scripting from outside | DAWproject |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **GarageBand** | **free** | Mac, iOS | good: a track per part, tempo map, meter, key, a patch per program (72 for 128) | no | `.band`, binary, undocumented | open, save, close (AppleScript) | no |
| **Logic Pro** 12 | $199.99 once, or Creator Studio $12.99/mo | Mac (Apple silicon), iPad | very good with *File › Open*: names, tempo, markers, a GarageBand patch per track | **yes** | `.logicx`, binary | little | no |
| **FL Studio** 26 | $99–$499; the trial never ends but **cannot reopen a saved project** | Win, Mac; Mobile $14.99 | a channel per track; FLEX plays GM programs; meters | no | `.flp`, binary | Python (controllers, piano roll) | no |
| **Ableton Live** 12 | Intro €79, Standard €279, Suite €599; Lite only bundled with hardware; 30-day trial | Win, Mac | clips on consecutive tracks; tempo only where notes span it; no instruments chosen | no | `.als`, gzipped XML, undocumented | Max for Live, Python remote scripts | no |
| **Reaper** 7.84 | $60 personal licence; the 60-day trial keeps working after, with a nag | Win, Mac, Linux | good: a track per source track, tempo map merge, markers; **no GM synth**, so silent until one is added | **yes** (since 5.23) | `.RPP`, **plain text** | Lua, EEL2, Python, command line, OSC | through ProjectConverter (LGPL) |
| **Bitwig Studio** 6 | Essentials €99, Producer €199, Studio €399; 8-Track bundled | Win, Mac, Linux | a track per MIDI track | no | binary; also opens `.als` and `.flp` | Java/JS controller API | **yes** |
| **Fender Studio Pro** 8 (Studio One until 2026-01) | $199.99 or $19.99/mo; the free *Fender Studio* is audio-only | Win, Mac, Linux beta | offers GM instruments; the tempo map may need dragging in | no | `.song`, zip of XML | none public | **yes** |
| **Cubase** 15 | Elements $99.99, Artist $329.99, Pro $579.99; LE/AI bundled | Win, Mac | good: instrument tracks with presets, first program, tempo, meter, markers | **yes** (14.0.20+) | `.cpr`, binary | MIDI Remote API (JS) | **yes** (Artist, Pro) |
| **Cakewalk** Sonar / Next | Sonar has a free tier; Next is **free** | Sonar Win; Next Win, Mac | [unverified] | [unverified] | binary | — | no |
| **Pro Tools Intro** | **free**, 8+8+8 tracks | Win, Mac | yes, with its tempo map | no | binary | — | no |
| **Waveform Free** 14 | **free**, no track limit | Win, Mac, Linux | [unverified] | no | XML | — | in Waveform 14; in Free [unverified] |
| **LMMS** 1.2.2 | **free**, GPL-2.0 | Win, Mac, Linux | splits by channel; tempo bugs; needs a SoundFont | no | XML | command line render | no |
| **Ardour** 9.8 | GPL-2.0; binaries from $1, distro packages free | Win, Mac, Linux | names from the file; tempo map optional | no | XML | Lua, OSC | no |
| **MPC Beats** | **free**, 2 audio + 8 MIDI tracks | Win, Mac | extra tracks dropped | no | — | — | no |
| **BandLab** | **free**, 16 tracks (32 paid), 15 min | web, iOS, Android | web yes; Android merges a format-1 file into one track | no | cloud | — | no |
| **Soundtrap** | free tier; paid ~$10–18/mo | web | at the playhead | no | cloud | a link that opens a hosted MIDI file [unverified] | no |
| **Audiotool**, **openDAW** | free; openDAW AGPL, a prototype | web | Audiotool in a 2026 beta; openDAW undocumented | no | cloud / zip | Nexus SDK; JS | openDAW's pre-release SDK |
| **MuseScore Studio** 4.7 (the notation side, for comparison) | **free**, GPL-3.0 | Win, Mac, Linux | into notation | **full** | `.mscz` | plugins, command line | no |

Sources, beyond the GarageBand note's measured §4.3: Logic
([price](https://www.apple.com/logic-pro/),
[Creator Studio](https://support.apple.com/en-mk/125029),
[opening a MIDI file](https://help.apple.com/logicpro/mac/10.1/en.lproj/lgcpdf6a3851.html),
[MusicXML](https://support.apple.com/en-kg/guide/logicpro/lgcp67fa6594/10.7/mac/11.0));
FL Studio ([trial](https://www.image-line.com/fl-studio/download),
[MIDI import](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/automation_midiimport.htm));
Ableton ([shop](https://www.ableton.com/en/shop/live/),
[Lite](https://www.ableton.com/en/products/live-lite/),
[tempo on import](https://help.ableton.com/hc/en-us/articles/360003387979));
Reaper ([purchase](https://www.reaper.fm/purchase.php),
[changelog](https://www.reaper.fm/whatsnew.txt),
[command line](https://github.com/ReaTeam/Doc/blob/master/REAPER-CLI.md));
Bitwig ([editions](https://www.bitwig.com/buy/),
[other DAWs' projects](https://www.bitwig.com/stories/transferring-projects-between-bitwig-studio-and-other-daws-333/));
Fender Studio Pro ([rename](https://www.soundonsound.com/news/studio-one-becomes-fender-studio-pro),
[Fender Studio](https://www.gearnews.com/fender-studio-daw-studio/));
Cubase ([MIDI import options](https://www.steinberg.help/r/cubase-elements/13.0/en/cubase_nuendo/topics/preferences/preferences_midi_midi_file_import_options_r.html),
[MusicXML back in 14.0.20](https://forums.steinberg.net/t/cubase-pro-14-removed-import-of-music-xml/950321),
[DAWproject](https://www.soundonsound.com/techniques/cubase-14-using-dawproject-exchange-projects-between-cubase-cubasis));
Cakewalk ([Next](https://cakewalk.com/next), [Sonar](https://www.cakewalk.com/sonar));
[Pro Tools Intro](https://cn.avid.com/pro-tools/intro);
[Waveform Free](https://www.tracktion.com/products/waveform-free);
[Ardour](https://ardour.org/whatsnew.html);
[BandLab limits](https://help.bandlab.com/hc/en-us/articles/115002945433-Track-and-Project-Duration-Limits);
[openDAW](https://github.com/andremichelle/openDAW);
[MuseScore 4.7](https://musescore.org/en/4.7).

### 2.2 What matters for an arrangement from a score

- **Every DAW takes a MIDI file and WAVs.** That route reaches all of them,
  and the studio already has it. What differs is how much of the file each
  one keeps. Notes, the tempo map and the meter come through nearly
  everywhere. The track names come through in most. A sound picked from the
  General MIDI program comes through only in GarageBand, Logic, FL Studio
  (FLEX), Cubase (presets) and Studio Pro (on a prompt). In Reaper, Ableton
  and Bitwig a part arrives silent or on a default synth.
- **Only the stems are placed by hand.** No DAW places a WAV from a MIDI
  file. In the GarageBand route the user drops them at bar 1 (D162).
- **A pickup moves the bars** in every DAW, since tick 0 is bar 1. D162's
  lead-in already fixes that for any DAW, not only GarageBand.
- **Free and good** narrows to GarageBand on the Mac, Cakewalk Next, Pro Tools
  Intro, Waveform Free on all three desktops, and BandLab in the browser.
  Reaper's $60 is nominal, since its trial never locks.
- **Sound for free.** GarageBand's library is the best that comes free with a
  DAW. MuseScore's Muse Sounds are the best free orchestral sounds, but
  MuseScore is a notation editor and holds no stems.

## 3. Which DAW to suggest, by platform

| User | Free pick | Why | Paid step up |
| --- | --- | --- | --- |
| **Mac** | **GarageBand** | installed, good sounds, already has its own export (D162) | **Logic Pro**: opens GarageBand projects, imports MusicXML, has a score editor and a stem splitter |
| **Windows** | **Cakewalk Next** or Waveform Free; Reaper "nearly free" | full DAWs at no cost; Reaper has the best import and a project file we can write | **Reaper** ($60), or Cubase Artist/Pro (MusicXML, DAWproject, score editor) |
| **Linux** | **Reaper** (nearly free) or **Waveform Free** | both native and complete; Ardour if it must be open source | Bitwig Studio (DAWproject) |
| **Browser only** | **BandLab** | free, MIDI and audio import on the web | — |

Avoid recommending, for this job: FL Studio's trial (a saved arrangement
cannot be reopened), Ableton Live Lite (8 tracks, bundled only, chooses no
sounds), MPC Beats (drops tracks past 8), LMMS (splits by channel, tempo bugs)
and the free Fender Studio (no MIDI).

## 4. What the studio could write

### 4.1 In short

| Format | Open? | Reaches | Keeps | Cost for us | Verdict |
| --- | --- | --- | --- | --- | --- |
| **A richer MIDI file** | standard | every DAW | names, programs, tempo, meter, key, markers, words | the GarageBand rewriter, generalised | **first** (§4.2) |
| **DAWproject** | MIT spec, XML in a zip | Bitwig, Studio Pro, Cubase/Nuendo/Cubasis, Waveform 14, VST Live, n-Track; Reaper by converter | tracks, names, notes, **stems placed**, tempo and meter maps, markers, volume, pan, mute | low to medium; a Rust crate exists | **second** (§4.3) |
| **Reaper `.RPP`** | plain text, community-documented | Reaper | all of it, stems and mix included; only the instrument is weak | low | **third** (§4.4) |
| **Final Cut Pro XML** | Apple-documented | Logic (stems only) | audio clips at their places, volume, pan | medium | only if Logic users ask |
| **MusicXML** | open | Logic, Cubase, Reaper, notation editors | notation: names, spelling, voices, words | medium; lilypond does not write it | only for notation exchange |
| **Ableton `.als`** | undocumented gzipped XML | Live (and Bitwig) | everything, in principle | high, changes with each version | no |
| **AAF** | binary container | Pro Tools, Logic, Cubase | audio only | high | no |
| **`.flp`, `.logicx`, `.band` from nothing** | binary, undocumented | — | — | — | no (D162 for `.band`) |

### 4.2 A richer MIDI file: one change, every DAW

D162's rewriter already names each part's track, puts it on a program, scales
to `% BPM:`, adds a lead-in after a pickup and drops what GarageBand turns
into empty tracks. Split into what is GarageBand's and what is anyone's, it
becomes an *Export for DAW…* whose MIDI file has:

- **FF 03, a track name per part.** Every DAW reads it, and most show it on
  the track, which GarageBand does not (D162).
- **A program change per part**, picked by the Parts fold. Logic, FL Studio's
  FLEX, Cubase, Studio Pro and GarageBand choose a sound from it.
- **The tempo map, meter and key** in track 0. These are already there.
- **FF 06 markers** at rehearsal marks or sections. Logic (*File › Open*),
  Reaper, Cubase and a new Studio Pro song read them
  ([Cubase](https://archive.steinberg.help/cubase_pro/v12/en/cubase_nuendo/topics/markers/markers_importing_markers_via_midi_c.html),
  [Studio One](https://marcus-huyskens.squarespace.com/mh-music-blog/importing-a-tempo-map-markers-from-pro-tools-into-your-studio-one-4-song)).
  lilypond writes none, so they would come from the playback map's bars and
  the score's marks. Whether the map holds the marks is to be checked.
- **FF 05 words on the voice's own track**, not a track of their own. Reaper
  shows them, and nothing else is harmed by them. GarageBand's export keeps
  dropping them (D162).
- **No GS or XG SysEx.** No DAW gains from it.

The WAVs stay D162's: aligned to tick 0, named by part. The folder is then
right for every DAW in §2, and the GarageBand export becomes one preset of it.

### 4.3 DAWproject

- **What it is.** An open exchange format, MIT, version 1.0, kept by Bitwig at
  [bitwig/dawproject](https://github.com/bitwig/dawproject). It is a zip of
  `project.xml`, `metadata.xml` and the audio. `project.xml` holds tracks with
  a channel each (volume, pan, mute, sends, devices), an arrangement of lanes
  and clips, notes, audio clips, tempo and time-signature automation, and
  markers ([Project.xsd](https://raw.githubusercontent.com/bitwig/dawproject/main/Project.xsd)).
- **Writers.** In Rust, [`dawproject`](https://github.com/bkbkb-net/dawproject)
  0.12.0 (MIT OR Apache-2.0, 2026-10-27, generated from the schema), or
  `quick-xml` and `zip` directly. In TypeScript,
  [`dawproject-typescript`](https://github.com/perivar/dawproject-typescript)
  (MIT), for the web build. DawVert is GPL-3.0: read it, never copy it.
- **What it gains over MIDI and WAVs.** The stems arrive placed, and the
  tracks arrive named, with the Parts fold's volume and mute as the channels'
  settings. That is everything D162's stage 5 needs a `.band` editor for,
  but for five DAWs and in an open format.
- **The gap: instruments.** A track cannot say "a piano". A device is either a
  vendor's built-in device or a VST3, CLAP or AU plug-in named by its ID, with
  its state as a file. So the studio would write no instrument, and the user
  picks one per track. Profiles that name a stock instrument for each DAW
  could come later.
- **Uneven in practice.** Cubase drops plain MIDI tracks, so the parts must be
  instrument tracks
  ([Steinberg forum](https://forums.steinberg.net/t/dawproject-export-no-midi-track-export/1009070)).
  Studio One to Cubase lost tempo changes
  ([Steinberg forum](https://forums.steinberg.net/t/dawproject-import-from-studio-one-unusable/953751)).
  Every target needs a check by hand with a score that has a tempo change, a
  meter change and a pickup, as GarageBand had (D165).

### 4.4 Reaper's `.RPP`

- **Plain text** of nested `<CHUNK …>` blocks, documented by the community in
  [State Chunk Definitions](https://github.com/ReaTeam/Doc/blob/master/State%20Chunk%20Definitions),
  current to 7.7x. A track is `<TRACK` with `NAME`, `VOLPAN` and `MUTESOLO`.
  An item is `<ITEM POSITION LENGTH` with a `<SOURCE WAVE FILE "…">`, or a
  `<SOURCE MIDI` with its events inline. The tempo and meter map is
  `TEMPOENVEX`, and markers are `MARKER` lines.
- **Keeps everything**: names, notes, stems placed, tempo and meter map,
  markers, volume, pan, mute. Reaper is $60, runs on all three desktops and
  is not reached by DAWproject without a converter.
- **The gap: no GM synth.** A MIDI track without an instrument is silent. On
  the Mac, Apple's `DLSMusicDevice` AU is a GM synth that Reaper loads
  ([report](https://cycling74.com/forums/general-midi-synth)). Whether an RPP
  can name it with no state chunk is [unverified] and is a spike's first
  question. Elsewhere the user adds an instrument per track.
- **Effort**: low. It is a text writer in `crates/session`, beside
  `midi_export.rs` and `wav_align.rs`. [`rpp-parser`](https://docs.rs/crate/rpp-parser/latest)
  (MIT/Apache) can read back what we write, for the tests.

### 4.5 What is not worth writing

- **`.als`.** It has no spec and changes with each Live version. Live Lite and
  Intro refuse to save a set with more tracks than they allow
  ([Ableton](https://help.ableton.com/hc/en-us/articles/209769925)). Live
  users get the MIDI file and WAVs.
- **AAF and OMF.** They carry audio only, in a binary container. Logic has
  FCP XML for the same job, and the others have DAWproject or RPP.
- **`.flp` and `.logicx`.** They are binary and undocumented, and the one
  `.flp` library is GPL. Logic opens the MIDI file better than it imports
  anything else.
- **MusicXML**, for now. It brings notation, not an arrangement. lilypond does
  not write it, and the studio would need its own writer
  ([`musicxml`](https://crates.io/crates/musicxml), MIT). It belongs to a
  notation-exchange feature, if one is wanted, not to this one.

## 5. A DAW inside Lily Studio

### 5.1 What it would be for

The score stays the source of the notes. Its arrangement is written in
LilyPond: which parts there are, and what they play. What a DAW adds on top is the sound and
the mix:

- a track per part, and one per stem and for the recording, on one timeline;
- a real instrument for each part, not D24's two oscillators;
- volume, pan, mute and solo per track, building on the Parts fold (D45,
  D121);
- the stems placed against the parts, as Export for GarageBand places them;
- a bounce to WAV, as Export Video already does with D140's
  `OfflineAudioContext`;
- perhaps a few effects (reverb, EQ, compression) per track.

The notes stay in the score. A piano roll could *show* a part, and a click
could move the editor to its line, but editing notes in a piano roll and
writing them back to LilyPond is a different and much larger project (the
visual-editor research). Recording, automation, plug-in hosting and mastering
are what the DAWs of §2 are for, and the export of §4 hands the work to them.

So "a DAW in the studio" is three pieces, worth more in this order:

1. **Better sound**: a General MIDI SoundFont in place of D24's synthesizer.
   It helps every score, the Parts fold, Karaoke and Export Video, even with
   no new view.
2. **A mixer and a timeline**: the Parts fold grown into tracks with the stems
   beside them, pan and solo added, and a bounce.
3. **Effects and plug-ins**, last, and perhaps never. Past that point the
   studio becomes a worse copy of GarageBand.

### 5.2 The building blocks

Licences were read on each project's repository (GitHub's licence field, then
the licence file). The apps are MIT, and GPL code may be read but not copied
(D162). That leaves:

| Block | What | Licence | Fit |
| --- | --- | --- | --- |
| [SpessaSynth](https://github.com/spessasus/spessasynth_lib) | SF2/SF3/DLS synth and MIDI player in an AudioWorklet; renders offline | Apache-2.0 | **use** (web, and the Mac WebView) |
| [rustysynth](https://github.com/sinshu/rustysynth) | SF2 synth and MIDI sequencer, pure Rust | MIT | **use** (native); reads SF2 only |
| [Signal](https://github.com/ryohey/signal) | web MIDI sequencer: piano roll, arrange view, SF2 worklet synth | MIT | **borrow or study** for a timeline and piano roll |
| [waveform-playlist](https://github.com/naomiaro/waveform-playlist) | multitrack audio clips, effects, WAV export, a MIDI lane | MIT | **use or study** for the stems' timeline |
| [wavesurfer.js](https://github.com/katspaugh/wavesurfer.js) | waveforms and regions | BSD-3-Clause | use, for waveforms only |
| [Tone.js](https://github.com/Tonejs/Tone.js) | transport, effects | MIT | use for effects, or study |
| [cpal](https://crates.io/crates/cpal), [symphonia](https://crates.io/crates/symphonia), [fundsp](https://crates.io/crates/fundsp), [creek](https://crates.io/crates/creek), [rtrb](https://crates.io/crates/rtrb) | device output, decoding (MPL-2.0, unmodified), DSP, disk streaming, real-time queues | MIT/Apache, MPL | use (native) |
| [objc2-avf-audio](https://crates.io/crates/objc2-avf-audio) | AVAudioEngine: AUMIDISynth or AVAudioUnitSampler, Apple's effects, offline render, the user's AU instruments | Zlib/Apache/MIT | use (native, Mac) |
| [clack](https://github.com/prokopyl/clack), [VST3 SDK](https://github.com/steinbergmedia/vst3sdk) | CLAP and VST3 hosting | MIT/Apache; VST3 MIT since 3.8 ([2025-10-29](https://sonicstate.com/news/2025/10/30/vst-3-now-available-under-mit-license)) | later, if ever |
| openDAW, GridSound, Tracktion Engine, Ardour, LMMS, Zrythm, JUCE | complete DAWs and engines | AGPL or GPL (JUCE AGPL or commercial) | **study only** |
| js-synthesizer (FluidSynth), OxiSynth | SoundFont synths | LGPL-2.1 | avoid |
| WebAudioFont | sampled instruments | GPL-3.0 | avoid |

### 5.3 The sound: a free General MIDI set

| Set | Licence | Size | For |
| --- | --- | --- | --- |
| **[GeneralUser GS](https://github.com/mrbumpy409/GeneralUser-GS)** 2.0.3 | its own, permissive: free to use in software and to repackage; do not hotlink the author's download ([LICENSE](https://github.com/mrbumpy409/GeneralUser-GS/blob/main/documentation/LICENSE.txt)) | **8.4 MB as SF3**, 32 MB as SF2 | **the default**, shipped in the app and served by the site |
| FluidR3Mono_GM | MIT | 23.7 MB SF3 | an optional download |
| MuseScore General / MS Basic | MIT, parts CC0 | 36–51 MB SF3 | an optional "better sound" download |
| VSCO-2 Community Edition | CC0 | 50 samples to ~3 GB | orchestral parts only; needs an SFZ player |
| Salamander Grand Piano | CC-BY 3.0 | ~400 MB | a piano, on demand only |
| macOS's `gs_instruments.dls` | Apple's; the macOS licence does not mention it | 2 MB | a zero-download fallback through Apple's own synth AU, never copied or loaded into ours |
| TimGM6mb | GPL-2.0 | 6 MB | avoid |

**[measured]** `/System/Library/Components/CoreAudio.component/Contents/Resources/gs_instruments.dls`
is present on macOS 26.5.2, 1 996 068 bytes. Apple's DLS synth AU plays it as
a full General MIDI set
([Apple](https://developer.apple.com/documentation/audiotoolbox/kaudiounitsubtype_dlssynth.md)).
It is an old QuickTime-era bank, and clearly worse than GeneralUser GS.

D24's reasons against a SoundFont were its size and its licence. An 8.4 MB
SF3 under a licence that invites use in software answers both. The
SoundFont's sample provenance is the one caveat: its author says the origin of
a few samples is uncertain, with no complaint since 2000.

### 5.4 Where the audio runs

Three ways to build it:

| | A. All in the WebView | B. A Rust engine, the WebView as UI | C. One Rust DSP crate, two hosts |
| --- | --- | --- | --- |
| **Synth** | SpessaSynth in an AudioWorklet | rustysynth behind cpal, or AVAudioEngine with AUMIDISynth | rustysynth native on the Mac, as Wasm in a worklet on the web |
| **Stems** | `AudioBufferSourceNode`s | symphonia and creek | native on the Mac; Web Audio on the web |
| **Bounce** | `OfflineAudioContext` | faster than real time, the same code | as B on the Mac |
| **Mac and web** | one code path, one sound | the web keeps today's player or gets A: two engines, two sounds | one sound, two builds |
| **WebKit risk** | **high**: D151's ring buffer turns any render stall into a lasting lag, and a heavier synth in a worklet stalls more; decoded stems cost ~63 MB each in memory | **none on the Mac**: Web Audio leaves the Mac app | none on the Mac; the web as A |
| **Licence risk** | low (Apache-2.0 notices) | low | low, if the crate stays in `packages/studio`, apart from the GPL engine (D56) |
| **Cost** | lowest | highest; the playhead reaches the UI as events | in between, plus a second wasm32 target |

Two facts decide between them:

- **D151.** The playback lag was WebKit's ring buffer, which keeps any stall of
  its render thread as a lasting delay. A SoundFont synth in an AudioWorklet
  renders on that same thread. Suspending the context while idle (D151) still
  helps, but a heavier render makes the stall more likely while the music
  plays. Moving the Mac app's audio out of the WebView removes the cause
  rather than working around it.
- **The web build has no stems and no recording** (D55, D155). On the web,
  A's sound improvement is the whole gain, and A's risks are lowest there:
  browsers other than Safari do not share WebKit's ring buffer.

So the sound can come first in **A** for both builds, which is cheap and
reversible. If the lag comes back with the heavier synth, or when the mixer of
§5.1 piece 2 is built, the Mac app moves to **B or C**. C keeps one sound on
both. Whether `crossOriginIsolated` holds on Tauri's `tauri://` scheme in
WKWebView, which a Wasm synth sharing memory with the UI would need, is
[unverified]. The engine spike ran without it (`wasm-engine/09-spike.md`).
SpessaSynth's worklet does not need it.

### 5.5 Size

Rough, from comparable MIT projects: Signal is ~40k lines of TypeScript for a
piano roll, an arrange view and a worklet synth. A read-only arranger (track
list, timeline of part clips and stem clips, read-only piano roll, mixer,
bounce) is ~5–10k lines of UI. Its engine is ~1.5–3k lines with SpessaSynth,
or ~3–5k in Rust. Editing notes in a piano roll is not in these numbers (§5.1).

## 6. Recommendation

**Hand arrangements to the user's DAW through better files, and give the
studio better sound. Do not build a DAW in the studio yet.**

- **Export for DAW** (§4.2): the GarageBand export generalised into a folder
  any DAW opens. It has a rich MIDI file, with names, programs, markers and
  the words on their part, plus the aligned WAVs. GarageBand stays a preset of
  it that opens GarageBand. It reaches every DAW in §2, the web included, at
  the cost of a refactor of code that exists.
- **DAWproject** (§4.3), then **Reaper's `.RPP`** (§4.4), as more choices
  in the same dialog. Both place the stems and carry the mix, which the MIDI
  route cannot. Each opens with no instrument chosen, so the dialog says so.
- **GeneralUser GS in the player** (§5.3), in place of D24's synthesizer,
  through SpessaSynth in both builds (§5.4, A). This is the part of "a DAW in
  the studio" that every user hears. It needs a decision that supersedes
  D24's *no SoundFont*.
- **The in-app mixer and timeline** (§5.1, piece 2) waits. It is worth
  building when users want to hear a finished arrangement inside the studio
  rather than in a DAW. It is the point to move the Mac app's audio to Rust
  (B or C), because of D151.
- **Never**: notes edited in a piano roll as a by-product of this work,
  plug-in hosting, or a GPL or AGPL engine linked in.

Suggest **GarageBand** to Mac users, **Cakewalk Next or Waveform Free** on
Windows, **Reaper or Waveform Free** on Linux and **BandLab** in a browser
(§3). The *Read me* of the export can say so.

## 7. A staged plan

Each stage stands alone.

1. **Export for DAW.** Split `garageBandPlan.ts` and `midi_export.rs`'s
   rewrite into a common plan and a GarageBand preset. Add FF 03 names on
   every track (already done for GarageBand), FF 06 markers and FF 05 words
   on the voice's track (both outside the GarageBand preset). The web gets
   the `.mid`. Check by hand in GarageBand (as D165), Reaper's trial, Cakewalk
   Next and Waveform Free, with a score that has a pickup, a tempo change, a
   meter change, a drum staff and words.
2. **Sound.** SpessaSynth and GeneralUser GS SF3 behind `midi.js`'s
   `Player`, keeping its parser and protocol, as D24 allowed. Check the D151
   lag with the RTAID method (memory of 2026-11-01) before and after, on the
   long idle sessions that showed it. Offer MS Basic as a download if users
   ask for better sound.
3. **DAWproject.** A writer in `crates/session`, with parts as instrument
   tracks with no device, the stems as audio clips at 0, and the Parts fold's
   volume and mute. Check by hand in Bitwig's and Cubase's trials, and Studio
   Pro if a licence is at hand.
4. **Reaper `.RPP`.** First a spike on whether `DLSMusicDevice` can be named
   in an RPP with no state, which would make the parts sound on the Mac.
   Then the writer, read back with `rpp-parser` in tests.
5. **Later, if users ask**: the in-app mixer and timeline, with the Mac
   app's audio in Rust (§5.4, B or C).

## 8. Open questions

- Does the playback map hold the score's rehearsal marks and sections, for
  FF 06 markers?
- Does Cakewalk Next import a format-1 MIDI file with names and GM sounds? Its
  manual says nothing.
- Do Bitwig and Cubase keep a DAWproject's tempo and meter changes and a
  pickup's lead-in?
- Can an RPP name Apple's `DLSMusicDevice` without a state chunk?
- Does SpessaSynth in WKWebView, while playing, add to D151's lag? Does
  `crossOriginIsolated` hold on `tauri://`?
- Does Soundtrap really open a MIDI file from a URL? If so, the web build
  could hand a score straight to a browser DAW.
