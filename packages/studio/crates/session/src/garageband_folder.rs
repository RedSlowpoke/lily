//! The folder of Export for GarageBand (D162, README §6 and §8 stage 2 of
//! `docs/research/garageband`): `<score> (GarageBand)/`, holding the MIDI
//! `for_garageband` rewrote, a WAV for each recording on the score's
//! timeline, aligned by `wav_align` so its sample 0 is the MIDI's tick 0, and
//! `Read me.txt`, which says how to open them and what does not arrive.
//!
//! The audio is the files the score's own lines name beside it (§6.3): the
//! `% Audio:` recording unless the Parts fold mutes it (D155), and
//! `% Backing:`, `% Music:` and `% Stem:`, which play on the score's own
//! time (D133, D135, D137). Each is shifted by the lead-in at the rewritten
//! first tempo, and the recording by its `% Audio offset:` as well. A WAV is
//! aligned as it is; any other file (FLAC and MP3 from karaoke-mlx, which
//! GarageBand refuses or delays, §4.4 and §1.5) is decoded by ffmpeg first,
//! and left out, and said to be, without it.
//!
//! The folder is built beside where it goes and put there only once whole, so
//! an export that fails leaves nothing, and a folder it replaces is kept
//! until then.
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::midi_export::lead_in_seconds;
use crate::wav_align::align_file;

/// What the score's lines name, as the renderer reads them (files.ts's
/// `audioOfScore` and the rest): each file by its name beside the score, and
/// what the Parts fold says of the recording.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreAudio {
    /// `% Audio:`, the recording played along with the MIDI (D155).
    #[serde(default)]
    pub audio: Option<String>,
    /// `% Audio offset:`, in seconds: the recording's place is the music's
    /// less it.
    #[serde(default)]
    pub audio_offset: f64,
    /// The Parts fold mutes the recording, which then stays out.
    #[serde(default)]
    pub audio_muted: bool,
    /// `% Backing:`, the backing track to sing over (D133).
    #[serde(default)]
    pub backing: Option<String>,
    /// `% Music:`, the music without any voice (D137).
    #[serde(default)]
    pub music: Option<String>,
    /// `% Stem:`, the stem the score was written from (D112).
    #[serde(default)]
    pub stem: Option<String>,
}

/// A file of the score's that goes into the folder.
#[derive(Clone, Debug, PartialEq)]
pub struct ChosenAudio {
    /// The file the score names.
    pub source: PathBuf,
    /// The WAV's name in the folder, which is its track's name in GarageBand.
    pub name: String,
    /// Seconds of silence put before it, or cut from its head when below 0.
    pub shift: f64,
}

/// The files of `audio` beside `score_dir`, in the order of §6.3, each named
/// and shifted: by `lead_in` seconds, and the recording by its offset too.
/// The muted recording is not chosen, and a file named twice for the same
/// place goes in once.
pub fn choose_audio(score_dir: &Path, audio: &ScoreAudio, lead_in: f64) -> Vec<ChosenAudio> {
    let recording = audio
        .audio
        .as_ref()
        .filter(|_| !audio.audio_muted)
        .map(|file| (file, lead_in + audio.audio_offset, true));
    let on_score_time = [&audio.backing, &audio.music, &audio.stem]
        .into_iter()
        .flatten()
        .map(|file| (file, lead_in, false));
    let mut taken = HashSet::new();
    let mut chosen: Vec<ChosenAudio> = Vec::new();
    for (file, shift, own_name) in recording.into_iter().chain(on_score_time) {
        let source = score_dir.join(file);
        if chosen
            .iter()
            .any(|other| other.source == source && other.shift == shift)
        {
            continue;
        }
        let name = free_name(&wav_name(&source, own_name), &mut taken);
        chosen.push(ChosenAudio {
            source,
            name,
            shift,
        });
    }
    chosen
}

/// What a file's WAV is called: a stem as karaoke-mlx names it,
/// `<song> - vocals.flac`, for what it is (`Vocals.wav`, README §4.6); the
/// recording, or any other file, by its own name.
pub fn wav_name(source: &Path, own_name: bool) -> String {
    let stem = source.file_stem().map_or_else(
        || "Audio".into(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    let what = stem
        .rsplit_once(" - ")
        .map(|(_, what)| what)
        .filter(|what| {
            !own_name
                && !what.is_empty()
                && what
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        });
    let name = match what {
        Some(what) => {
            let what = what.replace('_', " ");
            let mut chars = what.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        }
        // A name the Finder hides is no track's name.
        None => stem.trim_start_matches('.').to_string(),
    };
    if name.is_empty() {
        "Audio".into()
    } else {
        name
    }
}

/// `name` with `.wav`, and a number when another WAV of the folder has it:
/// the Mac's names ignore case.
fn free_name(name: &str, taken: &mut HashSet<String>) -> String {
    let mut count = 1;
    loop {
        let candidate = if count == 1 {
            format!("{name}.wav")
        } else {
            format!("{name} {count}.wav")
        };
        if taken.insert(candidate.to_lowercase()) {
            return candidate;
        }
        count += 1;
    }
}

/// `<score> (GarageBand)`, the folder's name for the score at `root_file`.
pub fn folder_name(root_file: &Path) -> String {
    format!("{} (GarageBand)", score_name(root_file))
}

fn score_name(root_file: &Path) -> String {
    root_file.file_stem().map_or_else(
        || "score".into(),
        |stem| stem.to_string_lossy().into_owned(),
    )
}

/// A file of the score's that the folder does not hold, and why.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LeftOut {
    /// Its name as the score has it.
    pub file: String,
    /// Why, as a sentence's end: "ffmpeg is not installed (brew install ffmpeg)".
    pub why: String,
}

/// What `write_folder` wrote.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WrittenFolder {
    pub folder: PathBuf,
    /// The `.mid` in it, for GarageBand to open.
    pub midi: PathBuf,
    /// The WAVs' names, in the order of §6.3.
    pub wavs: Vec<String>,
    /// The score's files that could not go in.
    pub left_out: Vec<LeftOut>,
}

/// What `write_folder` is given.
pub struct FolderExport<'a> {
    /// The score, whose name the folder and the `.mid` take and beside which
    /// its audio is.
    pub root_file: &'a Path,
    /// The MIDI `for_garageband` wrote.
    pub midi: &'a [u8],
    /// The plan's lead-in, in ticks.
    pub lead_in: u32,
    pub audio: &'a ScoreAudio,
    /// The muted parts D54's checkbox left out, by name, for the read-me.
    pub left_out_parts: &'a [String],
    /// ffmpeg, to decode what is not WAV; None when it is not installed.
    pub ffmpeg: Option<&'a Path>,
}

/// The export written to `folder`, a `<score> (GarageBand)` beside the score
/// or wherever the user chose. A folder there already is replaced only when
/// `replace` says so, the user having been asked; one that holds the score or
/// its audio never is. Audio that cannot go in is left out and said to be,
/// in the answer and in the read-me; anything else that fails fails the
/// export and leaves `folder` as it was.
pub fn write_folder(
    folder: &Path,
    export: &FolderExport,
    replace: bool,
) -> Result<WrittenFolder, String> {
    let shown = folder.file_name().map_or_else(
        || folder.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let parent = folder
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| format!("{shown} is in no folder."))?;
    let score_dir = export.root_file.parent().unwrap_or(Path::new(""));
    let chosen = choose_audio(
        score_dir,
        export.audio,
        lead_in_seconds(export.midi, export.lead_in)?,
    );
    if folder.symlink_metadata().is_ok() {
        if !replace {
            return Err(format!("{shown} is there already."));
        }
        let inside = folder
            .canonicalize()
            .map_err(|e| format!("{shown}: {e}."))?;
        let holds = |file: &Path| {
            file.canonicalize()
                .is_ok_and(|file| file.starts_with(&inside))
        };
        if holds(export.root_file) || chosen.iter().any(|audio| holds(&audio.source)) {
            return Err(format!(
                "{shown} holds the score or its audio, so it cannot be replaced."
            ));
        }
    }
    let building = parent.join(format!(".{shown}.partial"));
    // One an export that died left behind is ours.
    let _ = std::fs::remove_dir_all(&building);
    std::fs::create_dir(&building).map_err(|e| format!("{shown} could not be made: {e}."))?;
    let written = fill(&building, export, &chosen).and_then(|written| {
        if folder.symlink_metadata().is_ok() {
            remove(folder).map_err(|e| format!("{shown} could not be replaced: {e}."))?;
        }
        std::fs::rename(&building, folder)
            .map_err(|e| format!("{shown} could not be written: {e}."))?;
        Ok(written)
    });
    if written.is_err() {
        let _ = std::fs::remove_dir_all(&building);
    }
    let (wavs, left_out, midi_name) = written?;
    Ok(WrittenFolder {
        folder: folder.to_path_buf(),
        midi: folder.join(midi_name),
        wavs,
        left_out,
    })
}

/// A folder, or a file or link where one was to go.
fn remove(path: &Path) -> std::io::Result<()> {
    if path.symlink_metadata()?.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

/// The `.mid`, the WAVs and the read-me written into `dir`: the WAVs' names,
/// what was left out, and the `.mid`'s name.
fn fill(
    dir: &Path,
    export: &FolderExport,
    chosen: &[ChosenAudio],
) -> Result<(Vec<String>, Vec<LeftOut>, String), String> {
    let score = score_name(export.root_file);
    let midi_name = format!("{score}.mid");
    std::fs::write(dir.join(&midi_name), export.midi)
        .map_err(|e| format!("{midi_name} could not be written: {e}."))?;
    let mut wavs = Vec::new();
    let mut left_out = Vec::new();
    for (index, audio) in chosen.iter().enumerate() {
        let file = audio.source.file_name().map_or_else(
            || audio.source.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        match write_wav(dir, index, audio, export.ffmpeg) {
            Ok(()) => wavs.push(audio.name.clone()),
            Err(why) => left_out.push(LeftOut { file, why }),
        }
    }
    let readme = read_me(
        &score,
        &midi_name,
        &wavs,
        &left_out,
        export.left_out_parts,
        export.lead_in > 0,
    );
    std::fs::write(dir.join(READ_ME), readme)
        .map_err(|e| format!("{READ_ME} could not be written: {e}."))?;
    Ok((wavs, left_out, midi_name))
}

pub const READ_ME: &str = "Read me.txt";

/// `audio` aligned into `dir` under its name, through ffmpeg unless it is a
/// WAV. The error is why it was left out.
fn write_wav(
    dir: &Path,
    index: usize,
    audio: &ChosenAudio,
    ffmpeg: Option<&Path>,
) -> Result<(), String> {
    if !audio.source.is_file() {
        return Err("it is not beside the score".into());
    }
    let to = dir.join(&audio.name);
    if is_wav(&audio.source) {
        return align_file(&audio.source, &to, audio.shift)
            .map(drop)
            .map_err(reason);
    }
    let ffmpeg = ffmpeg.ok_or("ffmpeg is not installed (brew install ffmpeg)")?;
    let decoded = dir.join(format!(".decoding {index}.wav"));
    let aligned = decode(ffmpeg, &audio.source, &decoded).and_then(|()| {
        align_file(&decoded, &to, audio.shift)
            .map(drop)
            .map_err(reason)
    });
    let _ = std::fs::remove_file(&decoded);
    aligned
}

/// `align_file`'s sentence, "take.wav could not be aligned: it is
/// compressed (format 0x0002).", as the end of one that names the file
/// already: "it could not be aligned: …", without the name of the WAV ffmpeg
/// decoded into, and without the full stop the read-me and the status line
/// put after it.
fn reason(error: String) -> String {
    let error = error.trim_end_matches('.');
    match error.find(" could not be ") {
        Some(at) => format!("it{}", &error[at..]),
        None => error.to_string(),
    }
}

/// A WAV by its first bytes, whatever its extension.
fn is_wav(path: &Path) -> bool {
    let mut head = [0; 12];
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut head))
        .is_ok_and(|()| &head[0..4] == b"RIFF" && &head[8..12] == b"WAVE")
}

/// `from` decoded by ffmpeg into a WAV at `to`, at its own rate and channels.
/// A FLAC keeps its depth; a lossy file, which has none, is decoded to 24-bit,
/// so nothing of the decoder's is rounded off.
fn decode(ffmpeg: &Path, from: &Path, to: &Path) -> Result<(), String> {
    let codec = match flac_bits(from) {
        Some(bits) if bits <= 16 => "pcm_s16le",
        Some(bits) if bits > 24 => "pcm_s32le",
        _ => "pcm_s24le",
    };
    let output = Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-i"])
        .arg(from)
        .args([
            "-map",
            "0:a:0",
            "-map_metadata",
            "-1",
            "-c:a",
            codec,
            "-f",
            "wav",
        ])
        .arg(to)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("ffmpeg did not start ({e})"))?;
    if output.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&output.stderr);
    Err(
        match said.lines().rev().find(|line| !line.trim().is_empty()) {
            Some(line) => format!("ffmpeg could not decode it ({})", line.trim()),
            None => format!("ffmpeg could not decode it ({})", output.status),
        },
    )
}

/// A FLAC's bits a sample, from its STREAMINFO; None for any other file.
fn flac_bits(path: &Path) -> Option<u16> {
    let mut head = [0; 26];
    std::fs::File::open(path).ok()?.read_exact(&mut head).ok()?;
    // `fLaC`, the block's header (type 0 is STREAMINFO), then 10 bytes of
    // block and frame sizes, 20 bits of rate, 3 of channels and 5 of depth.
    if &head[0..4] != b"fLaC" || head[4] & 0x7F != 0 {
        return None;
    }
    let bits = ((u16::from(head[20]) & 0x01) << 4 | u16::from(head[21]) >> 4) + 1;
    Some(bits)
}

/// `Read me.txt`: the two steps of README §3.5, and what does not arrive
/// (§6.2), with the files and parts that were left out.
pub fn read_me(
    score: &str,
    midi: &str,
    wavs: &[String],
    left_out: &[LeftOut],
    left_out_parts: &[String],
    lead_in: bool,
) -> String {
    let mut text = format!("{score}: MIDI and audio for GarageBand\n\n");
    text += &format!(
        "1. Open \"{midi}\" in GarageBand (double-click it, or File > Open).\n   \
         That makes a project with the tempo map, the meter, the key and a\n   \
         track per part, each part's name on its regions.\n"
    );
    if !wavs.is_empty() {
        let files = match wavs {
            [one] => one.clone(),
            [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
            [] => unreachable!(),
        };
        let (them, together) = if wavs.len() == 1 {
            ("it", "")
        } else {
            ("them", " together")
        };
        text += &format!(
            "2. Select {files} in Finder and drag {them}{together}\n   \
             into the empty area below the tracks, at the very start of bar 1.\n\n\
             Every file starts at the same moment as the MIDI, so nothing else needs placing.\n"
        );
    }
    if lead_in {
        text += "The score's bar 1 is GarageBand's bar 2: bar 1 holds a lead-in and the pickup.\n";
    }
    text += "\nWhat does not arrive:\n\
             - The track headers' names: each shows its instrument, so two parts\n  \
             on strings both read String Ensemble. The part names are on the regions.\n\
             - The mix: the parts' volumes and pans, and the mutes";
    text += &match left_out_parts {
        [] => ".\n".to_string(),
        [one] => format!(" (the muted part {one} is left out).\n"),
        [rest @ .., last] => format!(
            " (the muted parts {} and {last} are left out).\n",
            rest.join(", ")
        ),
    };
    text += "- The words and the title.\n\
             - Instruments beyond the 72 of GarageBand's table: a part on one of\n  \
             them plays the one GarageBand maps it to.\n";
    if !left_out.is_empty() {
        text += "\nLeft out of this folder:\n";
        for file in left_out {
            text += &format!("- {}: {}.\n", file.file, file.why);
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED: &[u8] = include_bytes!("../tests/fixtures/garageband.expected.mid");
    /// The fixture's lead-in: two quarters at % BPM: 113.76.
    const LEAD_IN: u32 = 768;

    fn audio() -> ScoreAudio {
        ScoreAudio {
            audio: Some("Song (recorded).wav".into()),
            audio_offset: -2.4,
            audio_muted: false,
            backing: Some("Song - instrumental.flac".into()),
            music: None,
            stem: Some("Song - vocals.wav".into()),
        }
    }

    /// A 16-bit mono WAV of `frames` frames at `rate`, each sample its index.
    fn wav(frames: i16, rate: u32) -> Vec<u8> {
        let data: Vec<u8> = (0..frames).flat_map(i16::to_le_bytes).collect();
        let mut bytes = b"RIFF".to_vec();
        bytes.extend((36 + data.len() as u32).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16u32.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(rate.to_le_bytes());
        bytes.extend((rate * 2).to_le_bytes());
        bytes.extend(2u16.to_le_bytes());
        bytes.extend(16u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend((data.len() as u32).to_le_bytes());
        bytes.extend(data);
        bytes
    }

    /// A WAV's samples, 16-bit.
    fn samples(path: &Path) -> Vec<i16> {
        let bytes = std::fs::read(path).unwrap();
        let at = bytes.windows(4).position(|w| w == b"data").unwrap() + 8;
        bytes[at..]
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect()
    }

    #[test]
    fn names_each_file_for_what_it_is() {
        let name = |file: &str, own| wav_name(Path::new(file), own);
        assert_eq!(name("Song - vocals.wav", false), "Vocals");
        assert_eq!(
            name("Song - Live - instrumental.flac", false),
            "Instrumental"
        );
        assert_eq!(
            name("My Song - backing_vocals.mp3", false),
            "Backing vocals"
        );
        // Not karaoke-mlx's: the file's own name.
        assert_eq!(name("Song - Live.wav", false), "Song - Live");
        assert_eq!(name("bass.wav", false), "bass");
        assert_eq!(name(".hidden.wav", false), "hidden");
        // The recording keeps its own, whatever its shape.
        assert_eq!(name("Song - vocals.wav", true), "Song - vocals");
        assert_eq!(name("Song (recorded).mp3", true), "Song (recorded)");
    }

    #[test]
    fn chooses_the_recording_and_the_files_on_the_score_s_time() {
        let lead = 2.0 * 60.0 / 113.76;
        let chosen = choose_audio(Path::new("/s"), &audio(), lead);
        let got: Vec<(&str, &str, f64)> = chosen
            .iter()
            .map(|c| (c.source.to_str().unwrap(), c.name.as_str(), c.shift))
            .collect();
        assert_eq!(
            got,
            [
                ("/s/Song (recorded).wav", "Song (recorded).wav", lead - 2.4),
                ("/s/Song - instrumental.flac", "Instrumental.wav", lead),
                ("/s/Song - vocals.wav", "Vocals.wav", lead),
            ]
        );
        // Muted in the fold: the recording stays out; a positive offset waits.
        let muted = ScoreAudio {
            audio_muted: true,
            ..audio()
        };
        let names: Vec<String> = choose_audio(Path::new("/s"), &muted, lead)
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, ["Instrumental.wav", "Vocals.wav"]);
        let later = ScoreAudio {
            audio_offset: 1.25,
            backing: None,
            stem: None,
            ..audio()
        };
        assert_eq!(choose_audio(Path::new("/s"), &later, 0.0)[0].shift, 1.25);
        // The same file twice goes in once; two names alike get a number.
        let twice = ScoreAudio {
            audio: Some("a - vocals.wav".into()),
            audio_offset: 0.0,
            backing: Some("b - vocals.wav".into()),
            music: Some("b - vocals.wav".into()),
            stem: Some("c - Vocals.wav".into()),
            ..ScoreAudio::default()
        };
        let names: Vec<String> = choose_audio(Path::new("/s"), &twice, 0.0)
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, ["a - vocals.wav", "Vocals.wav", "c - Vocals.wav"]);
        let alike = ScoreAudio {
            backing: Some("x - vocals.wav".into()),
            stem: Some("y - vocals.flac".into()),
            ..ScoreAudio::default()
        };
        let names: Vec<String> = choose_audio(Path::new("/s"), &alike, 0.0)
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, ["Vocals.wav", "Vocals 2.wav"]);
        assert!(choose_audio(Path::new("/s"), &ScoreAudio::default(), lead).is_empty());
    }

    #[test]
    fn the_read_me_says_what_to_do_and_what_does_not_arrive() {
        let wavs = ["Song (recorded).wav".to_string(), "Vocals.wav".into()];
        let left = [LeftOut {
            file: "Song - instrumental.flac".into(),
            why: "ffmpeg is not installed (brew install ffmpeg)".into(),
        }];
        let text = read_me("Song", "Song.mid", &wavs, &left, &["Violin".into()], true);
        assert_eq!(
            text,
            "Song: MIDI and audio for GarageBand\n\
             \n\
             1. Open \"Song.mid\" in GarageBand (double-click it, or File > Open).\n   \
             That makes a project with the tempo map, the meter, the key and a\n   \
             track per part, each part's name on its regions.\n\
             2. Select Song (recorded).wav and Vocals.wav in Finder and drag them together\n   \
             into the empty area below the tracks, at the very start of bar 1.\n\
             \n\
             Every file starts at the same moment as the MIDI, so nothing else needs placing.\n\
             The score's bar 1 is GarageBand's bar 2: bar 1 holds a lead-in and the pickup.\n\
             \n\
             What does not arrive:\n\
             - The track headers' names: each shows its instrument, so two parts\n  \
             on strings both read String Ensemble. The part names are on the regions.\n\
             - The mix: the parts' volumes and pans, and the mutes (the muted part Violin is left out).\n\
             - The words and the title.\n\
             - Instruments beyond the 72 of GarageBand's table: a part on one of\n  \
             them plays the one GarageBand maps it to.\n\
             \n\
             Left out of this folder:\n\
             - Song - instrumental.flac: ffmpeg is not installed (brew install ffmpeg).\n"
        );
        // No audio, no pickup, nothing left out: one step, no lead-in, no list.
        let bare = read_me("Song", "Song.mid", &[], &[], &[], false);
        assert!(bare.contains("1. Open \"Song.mid\""));
        assert!(!bare.contains("2. Select"));
        assert!(!bare.contains("bar 2"));
        assert!(!bare.contains("Left out"));
        assert!(bare.contains("and the mutes.\n"));
        let one = read_me(
            "S",
            "S.mid",
            &["Vocals.wav".into()],
            &[],
            &["A".into(), "B".into(), "C".into()],
            false,
        );
        assert!(one.contains("2. Select Vocals.wav in Finder and drag it\n"));
        assert!(one.contains("(the muted parts A, B and C are left out)"));
    }

    /// A score with `audio()`'s files beside it: the recording and the vocals
    /// as WAV at 1 kHz, the instrumental as a FLAC (only its header).
    fn score_with_audio() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let score = dir.path().join("Song.ly");
        std::fs::write(&score, "\\version \"2.24.0\"\n").unwrap();
        std::fs::write(dir.path().join("Song (recorded).wav"), wav(4000, 1000)).unwrap();
        std::fs::write(dir.path().join("Song - vocals.wav"), wav(10, 1000)).unwrap();
        std::fs::write(
            dir.path().join("Song - instrumental.flac"),
            b"fLaC\0\0\0\x22",
        )
        .unwrap();
        (dir, score)
    }

    #[test]
    fn writes_the_folder_and_leaves_out_a_flac_without_ffmpeg() {
        let (dir, score) = score_with_audio();
        let audio = audio();
        let export = FolderExport {
            root_file: &score,
            midi: EXPECTED,
            lead_in: LEAD_IN,
            audio: &audio,
            left_out_parts: &[],
            ffmpeg: None,
        };
        let folder = dir.path().join(folder_name(&score));
        assert_eq!(folder.file_name().unwrap(), "Song (GarageBand)");
        let written = write_folder(&folder, &export, false).unwrap();
        assert_eq!(written.midi, folder.join("Song.mid"));
        assert_eq!(written.wavs, ["Song (recorded).wav", "Vocals.wav"]);
        assert_eq!(
            written.left_out,
            [LeftOut {
                file: "Song - instrumental.flac".into(),
                why: "ffmpeg is not installed (brew install ffmpeg)".into(),
            }]
        );
        let mut files: Vec<String> = std::fs::read_dir(&folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        assert_eq!(
            files,
            [READ_ME, "Song (recorded).wav", "Song.mid", "Vocals.wav"]
        );
        assert_eq!(std::fs::read(folder.join("Song.mid")).unwrap(), EXPECTED);
        // The lead-in is 1.0549 s, 1055 frames at 1 kHz. The recording's
        // offset of -2.4 s makes it a cut of 1345 frames.
        let recording = samples(&folder.join("Song (recorded).wav"));
        assert_eq!(recording.len(), 4000 - 1345);
        assert_eq!(recording[0], 1345);
        let vocals = samples(&folder.join("Vocals.wav"));
        assert_eq!(vocals.len(), 1055 + 10);
        assert!(vocals[..1055].iter().all(|&s| s == 0));
        assert_eq!(vocals[1055..], [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let readme = std::fs::read_to_string(folder.join(READ_ME)).unwrap();
        assert!(readme.contains("Select Song (recorded).wav and Vocals.wav in Finder"));
        assert!(readme.contains(
            "- Song - instrumental.flac: ffmpeg is not installed (brew install ffmpeg).\n"
        ));
        // Nothing of the build is left beside it.
        assert!(!dir.path().join(".Song (GarageBand).partial").exists());
    }

    #[test]
    fn says_why_a_wav_the_aligner_refuses_is_left_out() {
        let (dir, score) = score_with_audio();
        let mut compressed = wav(10, 1000);
        // Format 2, ADPCM.
        compressed[20] = 2;
        std::fs::write(dir.path().join("Song - vocals.wav"), compressed).unwrap();
        let audio = ScoreAudio {
            backing: None,
            ..audio()
        };
        let export = FolderExport {
            root_file: &score,
            midi: EXPECTED,
            lead_in: LEAD_IN,
            audio: &audio,
            left_out_parts: &[],
            ffmpeg: None,
        };
        let folder = dir.path().join(folder_name(&score));
        let written = write_folder(&folder, &export, false).unwrap();
        assert_eq!(written.wavs, ["Song (recorded).wav"]);
        let why = "it could not be aligned: it is compressed (format 0x0002)";
        assert_eq!(
            written.left_out,
            [LeftOut {
                file: "Song - vocals.wav".into(),
                why: why.into(),
            }]
        );
        let readme = std::fs::read_to_string(folder.join(READ_ME)).unwrap();
        assert!(readme.contains(&format!("- Song - vocals.wav: {why}.\n")));
        assert_eq!(
            reason(".decoding 0.wav could not be read: gone.".into()),
            "it could not be read: gone"
        );
    }

    #[test]
    fn replaces_a_folder_only_when_told_and_never_the_score_s() {
        let (dir, score) = score_with_audio();
        let audio = ScoreAudio::default();
        let export = FolderExport {
            root_file: &score,
            midi: EXPECTED,
            lead_in: LEAD_IN,
            audio: &audio,
            left_out_parts: &["Violin".into()],
            ffmpeg: None,
        };
        let folder = dir.path().join("Song (GarageBand)");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("old.txt"), "old").unwrap();
        assert_eq!(
            write_folder(&folder, &export, false).unwrap_err(),
            "Song (GarageBand) is there already."
        );
        assert!(folder.join("old.txt").exists());
        let written = write_folder(&folder, &export, true).unwrap();
        assert!(written.wavs.is_empty() && written.left_out.is_empty());
        assert!(!folder.join("old.txt").exists());
        assert!(
            std::fs::read_to_string(folder.join(READ_ME))
                .unwrap()
                .contains("(the muted part Violin is left out)")
        );
        // The score's own folder is not replaced, even when told.
        let home = dir.path().join("Home (GarageBand)");
        std::fs::create_dir(&home).unwrap();
        let inner = home.join("Song.ly");
        std::fs::write(&inner, "").unwrap();
        let export = FolderExport {
            root_file: &inner,
            ..export
        };
        assert!(write_folder(&home, &export, true).is_err());
        assert!(inner.exists());
        // A MIDI that is not one fails the export and leaves the folder.
        let bad = FolderExport {
            midi: b"not midi",
            ..export
        };
        assert!(write_folder(&folder, &bad, true).is_err());
        assert!(folder.join("Song.mid").exists());
    }

    #[test]
    fn reads_a_flac_s_depth() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.flac");
        // STREAMINFO of 44.1 kHz stereo at 24 bits: rate 0x0AC44, channels 1, bits 23.
        let mut head = b"fLaC\0\0\0\x22".to_vec();
        head.extend([0x10, 0, 0x10, 0, 0, 0, 0, 0, 0, 0]);
        head.extend([0x0A, 0xC4, 0x43, 0x70, 0, 0, 0, 0]);
        std::fs::write(&path, &head).unwrap();
        assert_eq!(flac_bits(&path), Some(24));
        head[20] = 0x42;
        head[21] = 0xF0;
        std::fs::write(&path, &head).unwrap();
        assert_eq!(flac_bits(&path), Some(16));
        std::fs::write(&path, wav(4, 1000)).unwrap();
        assert_eq!(flac_bits(&path), None);
    }

    /// ffmpeg on the PATH or in Homebrew's folders, for the tests that decode.
    fn ffmpeg() -> Option<PathBuf> {
        let path = std::env::var("PATH").unwrap_or_default();
        format!("/opt/homebrew/bin:/usr/local/bin:{path}")
            .split(':')
            .map(|dir| Path::new(dir).join("ffmpeg"))
            .find(|path| path.is_file())
    }

    #[test]
    fn decodes_a_flac_through_ffmpeg() {
        let Some(ffmpeg) = ffmpeg() else {
            eprintln!("skipped: ffmpeg is not installed");
            return;
        };
        let (dir, score) = score_with_audio();
        let flac = dir.path().join("Song - instrumental.flac");
        let status = Command::new(&ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(dir.path().join("Song - vocals.wav"))
            .arg(&flac)
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(flac_bits(&flac), Some(16));
        let audio = ScoreAudio {
            backing: Some("Song - instrumental.flac".into()),
            ..ScoreAudio::default()
        };
        let export = FolderExport {
            root_file: &score,
            midi: EXPECTED,
            lead_in: LEAD_IN,
            audio: &audio,
            left_out_parts: &[],
            ffmpeg: Some(&ffmpeg),
        };
        let folder = dir.path().join("Song (GarageBand)");
        let written = write_folder(&folder, &export, false).unwrap();
        assert_eq!(written.wavs, ["Instrumental.wav"]);
        assert!(written.left_out.is_empty());
        let samples = samples(&folder.join("Instrumental.wav"));
        assert_eq!(samples.len(), 1055 + 10);
        assert_eq!(samples[1055..], [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        // What ffmpeg cannot read is left out, and why is said.
        std::fs::write(&flac, b"fLaC\0\0\0\x22 not really").unwrap();
        let written = write_folder(&folder, &export, true).unwrap();
        assert!(written.wavs.is_empty());
        assert!(
            written.left_out[0]
                .why
                .starts_with("ffmpeg could not decode it (")
        );
        assert!(!folder.join(".decoding 0.wav").exists());
    }
}
