//! Export Video (DECISIONS D140): Karaoke as an MP4 file. The page draws the
//! frames one by one, faster or slower than they play, and sends each as a
//! JPEG; ffmpeg encodes them into a silent video in the temp directory. The
//! melody the page rendered comes in chunks as a WAV beside it. Finishing
//! puts the video, the recordings the page chose (the backing track, the
//! score's audio track at its offset and volume, D155) and the melody into a hidden file beside
//! the one the save panel named, and renames it over that one when ffmpeg
//! succeeds. One export at a time; the web server has none (D55).
//!
//! The state's lock is held only for moments, never across a write or a
//! wait: a task of its own owns each ffmpeg, reads its stderr as it goes and
//! kills it when told, so a cancel, or the app's exit, never waits behind a
//! frame that ffmpeg is not reading; the frame's write fails instead.
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, MutexGuard};

use base64::Engine as _;
use lily_session::Studio;
use serde::Deserialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, State};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{ChildStdin, Command};
use tokio::sync::{Mutex, oneshot};

use crate::dialogs::{self, Save};
use crate::smoke::Smoke;

type Answer<T> = Result<T, String>;

const NO_VIDEO: &str = "No video is being exported.";

/// How much of what ffmpeg says on stderr is kept for a failure's message.
const TAIL: usize = 8 * 1024;

/// The last [`TAIL`] bytes of what a process said, from the start of a line
/// when one starts in them.
#[derive(Default)]
struct Tail(Vec<u8>);

impl Tail {
    fn push(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
        if self.0.len() <= TAIL {
            return;
        }
        let cut = self.0.len() - TAIL;
        let cut = self.0[cut..]
            .iter()
            .position(|&byte| byte == b'\n')
            .map(|at| cut + at + 1)
            .filter(|&at| at < self.0.len())
            .unwrap_or(cut);
        self.0.drain(..cut);
    }
}

/// Reads `from` to its end, keeping its tail.
async fn drain(mut from: impl AsyncRead + Unpin) -> Vec<u8> {
    let mut tail = Tail::default();
    let mut buffer = [0; 4096];
    loop {
        match from.read(&mut buffer).await {
            Ok(0) | Err(_) => return tail.0,
            Ok(read) => tail.push(&buffer[..read]),
        }
    }
}

/// How an ffmpeg ended.
struct Ended {
    status: std::io::Result<ExitStatus>,
    /// It was killed: the export was cancelled or discarded.
    killed: bool,
    /// The tail of its stderr.
    said: Vec<u8>,
}

impl Ended {
    fn check(&self) -> Answer<()> {
        match &self.status {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(failure(&self.said, *status)),
            Err(error) => Err(format!("ffmpeg failed: {error}")),
        }
    }
}

/// An ffmpeg running. A task of its own waits for it, draining its stderr
/// so that it never blocks writing there, and kills it when [`kill`] is
/// called or this is dropped.
///
/// [`kill`]: Process::kill
struct Process {
    kill: Option<oneshot::Sender<oneshot::Sender<()>>>,
    /// How it ended, for the one who waits for it.
    ended: Option<oneshot::Receiver<Ended>>,
}

impl Process {
    /// Starts `program`; its stdin is answered when `input`, else it is null.
    fn spawn(
        program: &Path,
        args: Vec<OsString>,
        input: bool,
    ) -> std::io::Result<(Process, Option<ChildStdin>)> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(if input { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let stdin = child.stdin.take();
        let stderr = child.stderr.take();
        let (kill, killed) = oneshot::channel::<oneshot::Sender<()>>();
        let (report, ended) = oneshot::channel();
        tokio::spawn(async move {
            let said = tokio::spawn(async move {
                match stderr {
                    Some(stderr) => drain(stderr).await,
                    None => Vec::new(),
                }
            });
            // An order to kill, or the handle dropped, which is one too.
            let order = tokio::select! {
                status = child.wait() => Ok(status),
                order = killed => Err(order.ok()),
            };
            let (status, killed) = match order {
                Ok(status) => (status, false),
                Err(done) => {
                    let _ = child.start_kill();
                    let status = child.wait().await;
                    if let Some(done) = done {
                        let _ = done.send(());
                    }
                    (status, true)
                }
            };
            let said = said.await.unwrap_or_default();
            let _ = report.send(Ended {
                status,
                killed,
                said,
            });
        });
        Ok((
            Process {
                kill: Some(kill),
                ended: Some(ended),
            },
            stdin,
        ))
    }

    /// Kills it if it still runs; answers once it is gone.
    async fn kill(&mut self) {
        if let Some(kill) = self.kill.take() {
            let (done, gone) = oneshot::channel();
            if kill.send(done).is_ok() {
                let _ = gone.await;
            }
        }
    }
}

/// Waits for an ffmpeg to end.
async fn ended(ended: Option<oneshot::Receiver<Ended>>) -> Ended {
    let gone = || Ended {
        status: Err(std::io::Error::other("ffmpeg was not waited for")),
        killed: true,
        said: Vec::new(),
    };
    match ended {
        Some(ended) => ended.await.unwrap_or_else(|_| gone()),
        None => gone(),
    }
}

/// The export running, if one is.
#[derive(Default)]
pub struct Video(std::sync::Mutex<Option<Job>>);

struct Job {
    /// Tells this export from one started after it.
    id: u64,
    ffmpeg: PathBuf,
    /// Encoding the frames, then putting the video and the sound together.
    process: Process,
    /// The encoder's stdin until the frames end. A frame's write holds this
    /// lock, never the [`Video`]'s.
    frames: Arc<Mutex<Option<ChildStdin>>>,
    /// In the temp directory: the silent video and the melody.
    dir: PathBuf,
    output: PathBuf,
}

impl Job {
    fn video(&self) -> PathBuf {
        self.dir.join("video.mp4")
    }

    fn melody(&self) -> PathBuf {
        self.dir.join("melody.wav")
    }

    /// Kills its ffmpeg and deletes what it wrote: the temp files, and the
    /// output's part file if the last step had begun.
    async fn discard(mut self) {
        self.process.kill().await;
        let _ = tokio::fs::remove_file(part_path(&self.output)).await;
        let _ = tokio::fs::remove_dir_all(&self.dir).await;
    }
}

impl Video {
    fn slot(&self) -> MutexGuard<'_, Option<Job>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Makes `job` the export running. One running still is one the page
    /// lost track of, as when it reloads mid-export, since the page runs one
    /// at a time and the app has one window (D123): it is discarded.
    async fn begin(&self, job: Job) {
        let stale = self.slot().replace(job);
        if let Some(stale) = stale {
            stale.discard().await;
        }
    }

    fn is_running(&self, id: u64) -> bool {
        self.slot().as_ref().is_some_and(|job| job.id == id)
    }

    /// Export `id`, out of the state, unless it was cancelled or replaced.
    fn take(&self, id: u64) -> Option<Job> {
        let mut slot = self.slot();
        if slot.as_ref().is_some_and(|job| job.id == id) {
            slot.take()
        } else {
            None
        }
    }

    /// Stops the export running at any step, its ffmpeg killed and its
    /// files gone; false when none was.
    pub async fn cancel(&self) -> bool {
        let job = self.slot().take();
        let running = job.is_some();
        if let Some(job) = job {
            job.discard().await;
        }
        running
    }
}

/// `<score> (karaoke).mp4`.
fn suggested_name(root_file: &Path) -> String {
    let stem = root_file
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "score".into());
    format!("{stem} (karaoke).mp4")
}

/// ffmpeg in Homebrew's folders or on the studio's search path: Export
/// Video's encoder, and Export for GarageBand's decoder of what is not WAV
/// (lily_session::garageband_folder).
pub(crate) fn find_ffmpeg(search_path: &str) -> Answer<PathBuf> {
    lily_separate::tool_path(search_path)
        .split(':')
        .map(|dir| Path::new(dir).join("ffmpeg"))
        .find(|path| path.is_file())
        .ok_or_else(|| "ffmpeg is not installed: brew install ffmpeg".into())
}

/// JPEG frames from stdin, `fps` a second, into H.264 at `video`.
fn encode_args(fps: u32, video: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "image2pipe",
        "-framerate",
        &fps.to_string(),
        "-c:v",
        "mjpeg",
        "-i",
        "-",
        "-c:v",
        "libx264",
        "-preset",
        "medium",
        "-crf",
        "20",
        "-pix_fmt",
        "yuv420p",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    args.push(video.into());
    args
}

/// Where the last step writes before the file is renamed over `output`:
/// hidden, beside it so that the rename stays in one folder, and still an
/// `.mp4`.
fn part_path(output: &Path) -> PathBuf {
    let stem = output
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "video".into());
    output.with_file_name(format!(".{stem}.part.mp4"))
}

/// A sound to put in the video: a file that starts `offset` s into the
/// score, at `volume` (1 as it is), D155.
#[derive(Debug, Clone, Copy)]
struct Input<'a> {
    path: &'a Path,
    offset: f64,
    volume: f64,
}

/// A recording the page chose for the video, as it sends it (D155).
#[derive(Debug, Deserialize)]
pub struct Recording {
    path: PathBuf,
    /// Where in the score it starts, in seconds.
    offset: f64,
    /// 0–1, as it is played.
    volume: f64,
}

/// The sound moved to the video's start, `start` s into the score (D149):
/// cut before it, or below 0 after as much silence; None at 0.
fn shift(start: f64) -> Option<String> {
    if start > 0.0 {
        Some(format!("atrim=start={start:.3},asetpts=PTS-STARTPTS"))
    } else if start < 0.0 {
        Some(format!(
            "adelay=delays={}:all=1",
            (-start * 1000.0).round() as u64
        ))
    } else {
        None
    }
}

/// What ffmpeg does to an input before it is summed: moved to the video's
/// start, `start` s into the score, from its own offset, and made louder or
/// softer; None when nothing.
fn input_filter(input: &Input, start: f64) -> Option<String> {
    let mut steps: Vec<String> = shift(start - input.offset).into_iter().collect();
    if input.volume != 1.0 {
        steps.push(format!("volume={:.3}", input.volume));
    }
    (!steps.is_empty()).then(|| steps.join(","))
}

/// `video` with `audio` (each from where it starts in the score, moved to
/// the video's `start`, summed as they are heard), into `output` as an MP4
/// lasting `seconds` when given, the video copied as it is.
fn mux_args(
    video: &Path,
    audio: &[Input],
    start: f64,
    seconds: Option<f64>,
    output: &Path,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-y", "-i"]
        .iter()
        .map(OsString::from)
        .collect();
    args.push(video.into());
    for input in audio {
        args.extend(["-i".into(), input.path.into()]);
    }
    let filters: Vec<Option<String>> = audio
        .iter()
        .map(|input| input_filter(input, start))
        .collect();
    let filter = match audio.len() {
        0 => None,
        1 => filters[0]
            .as_ref()
            .map(|filter| format!("[1:a]{filter}[a]")),
        count => {
            let mut graph = String::new();
            let mut mix = String::new();
            for (index, filter) in filters.iter().enumerate() {
                let n = index + 1;
                match filter {
                    Some(filter) => {
                        graph.push_str(&format!("[{n}:a]{filter}[s{n}];"));
                        mix.push_str(&format!("[s{n}]"));
                    }
                    None => mix.push_str(&format!("[{n}:a]")),
                }
            }
            Some(format!(
                "{graph}{mix}amix=inputs={count}:duration=longest:normalize=0[a]"
            ))
        }
    };
    let more: Vec<String> = match (&filter, audio.len()) {
        (_, 0) => vec!["-map".into(), "0:v".into()],
        (None, _) => vec!["-map".into(), "0:v".into(), "-map".into(), "1:a".into()],
        (Some(filter), _) => vec![
            "-filter_complex".into(),
            filter.clone(),
            "-map".into(),
            "0:v".into(),
            "-map".into(),
            "[a]".into(),
        ],
    };
    args.extend(more.into_iter().map(OsString::from));
    // A recording that goes on past the frames is cut where they end.
    if let Some(seconds) = seconds {
        args.extend(["-t".into(), format!("{seconds:.3}")].map(OsString::from));
    }
    args.extend(["-c:v", "copy"].map(OsString::from));
    if !audio.is_empty() {
        args.extend(["-c:a", "aac", "-b:a", "192k"].map(OsString::from));
    }
    args.extend(["-movflags", "+faststart", "-f", "mp4"].map(OsString::from));
    args.push(output.into());
    args
}

fn decode(data: &str) -> Answer<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|error| format!("The video could not be exported: {error}"))
}

/// What ffmpeg said, or how it ended.
fn failure(stderr: &[u8], status: std::process::ExitStatus) -> String {
    let said = String::from_utf8_lossy(stderr).trim().to_string();
    if said.is_empty() {
        format!("ffmpeg failed ({status}).")
    } else {
        format!("ffmpeg failed: {said}")
    }
}

/// The frames' size, in pixels, and how many a second.
#[derive(Deserialize)]
pub struct Size {
    width: u32,
    height: u32,
    fps: u32,
}

/// Asks where to save the video of `root_file` and starts ffmpeg for
/// frames of `width` × `height`; None when the panel was cancelled.
#[tauri::command]
pub async fn video_start(
    app: AppHandle,
    studio: State<'_, Arc<Studio>>,
    smoke: State<'_, Smoke>,
    video: State<'_, Video>,
    root_file: Value,
    size: Size,
) -> Answer<Option<PathBuf>> {
    let Size { width, height, fps } = size;
    let root = studio.check(&root_file)?;
    // H.264's 4:2:0 wants even sides.
    if !(2..=3840).contains(&width) || !(2..=2160).contains(&height) || width % 2 + height % 2 > 0 {
        return Err(format!("A video cannot be {width} × {height}."));
    }
    if !(1..=60).contains(&fps) {
        return Err(format!("A video cannot have {fps} frames a second."));
    }
    // An export still running is one the page lost track of (see `begin`).
    video.cancel().await;
    let ffmpeg = find_ffmpeg(&studio.search_path.get())?;
    let directory = root.parent().map(Path::to_path_buf);
    let name = suggested_name(&root);
    let output = match (&smoke.0, directory) {
        // The smoke test answers no panels: it takes the suggestion.
        (Some(_), Some(directory)) => Some(directory.join(name)),
        (_, directory) => {
            dialogs::save(
                &app,
                Save {
                    title: "Export Video".into(),
                    prompt: Some("Export".into()),
                    directory,
                    name: Some(name),
                    extensions: vec!["mp4".into()],
                    checkbox: None,
                },
            )
            .await?
        }
    };
    let Some(output) = output else {
        return Ok(None);
    };
    static RUNS: AtomicU64 = AtomicU64::new(0);
    let id = RUNS.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("lily-video-{}-{id}", std::process::id()));
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|error| format!("{}: {error}", dir.display()))?;
    let (process, frames) =
        match Process::spawn(&ffmpeg, encode_args(fps, &dir.join("video.mp4")), true) {
            Ok((process, Some(frames))) => (process, frames),
            Ok((mut process, None)) => {
                process.kill().await;
                let _ = tokio::fs::remove_dir_all(&dir).await;
                return Err("ffmpeg took no frames.".into());
            }
            Err(error) => {
                let _ = tokio::fs::remove_dir_all(&dir).await;
                return Err(format!("ffmpeg did not start: {error}"));
            }
        };
    video
        .begin(Job {
            id,
            ffmpeg,
            process,
            frames: Arc::new(Mutex::new(Some(frames))),
            dir,
            output: output.clone(),
        })
        .await;
    Ok(Some(output))
}

/// The next frame, a JPEG as base64.
#[tauri::command]
pub async fn video_frame(video: State<'_, Video>, frame: String) -> Answer<()> {
    let bytes = decode(&frame)?;
    let (id, frames) = {
        let slot = video.slot();
        let job = slot.as_ref().ok_or(NO_VIDEO)?;
        (job.id, job.frames.clone())
    };
    let written = match frames.lock().await.as_mut() {
        Some(frames) => frames.write_all(&bytes).await.is_ok(),
        None => false,
    };
    if written {
        return Ok(());
    }
    // ffmpeg stopped reading: killed by a cancel, or it says why as it ends.
    let Some(mut job) = video.take(id) else {
        return Err("The export was stopped.".into());
    };
    job.frames.lock().await.take();
    let encoded = ended(job.process.ended.take()).await;
    job.discard().await;
    Err(encoded
        .check()
        .err()
        .unwrap_or_else(|| "ffmpeg stopped taking frames.".into()))
}

/// The next part of the melody's WAV, as base64.
#[tauri::command]
pub async fn video_audio(video: State<'_, Video>, chunk: String) -> Answer<()> {
    let bytes = decode(&chunk)?;
    let melody = video.slot().as_ref().ok_or(NO_VIDEO)?.melody();
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&melody)
        .await
        .map_err(|error| format!("{}: {error}", melody.display()))?;
    file.write_all(&bytes)
        .await
        .map_err(|error| format!("{}: {error}", melody.display()))
}

/// Ends the frames and writes the video with `recordings`, the ones the page
/// plays and chose (D155), and the melody if any was sent, the sound from
/// `start` s of the score (D149) for `seconds`; answers the file written, or
/// None when the export was cancelled meanwhile.
#[tauri::command]
pub async fn video_finish(
    app: AppHandle,
    video: State<'_, Video>,
    recordings: Vec<Recording>,
    start: Option<f64>,
    seconds: Option<f64>,
) -> Answer<Option<PathBuf>> {
    let id = video.slot().as_ref().ok_or(NO_VIDEO)?.id;
    let start = start.unwrap_or(0.0);
    let refused = recordings.iter().find_map(|recording| {
        if !app.asset_protocol_scope().is_allowed(&recording.path) {
            Some(format!(
                "{} is not a recording the studio plays.",
                recording.path.display()
            ))
        } else if !(-86_400.0..=86_400.0).contains(&recording.offset)
            || !(0.0..=1.0).contains(&recording.volume)
        {
            Some(format!(
                "{} cannot start at {} s at {}.",
                recording.path.display(),
                recording.offset,
                recording.volume
            ))
        } else {
            None
        }
    });
    let result = match refused {
        Some(refused) => Err(refused),
        _ if !(-60.0..=86_400.0).contains(&start) => {
            Err(format!("A video cannot start at {start} s."))
        }
        _ if seconds.is_some_and(|seconds| !(seconds > 0.0 && seconds <= 86_400.0)) => {
            Err("A video cannot last that long.".into())
        }
        _ => finish(&video, id, &recordings, start, seconds).await,
    };
    // What is left of it: the temp files, and the part file of a failure.
    if let Some(job) = video.take(id) {
        job.discard().await;
    }
    result
}

/// The last step, which stays in the state so that a cancel can kill it.
async fn finish(
    video: &Video,
    id: u64,
    recordings: &[Recording],
    start: f64,
    seconds: Option<f64>,
) -> Answer<Option<PathBuf>> {
    let (frames, encoder) = {
        let mut slot = video.slot();
        let Some(job) = slot.as_mut().filter(|job| job.id == id) else {
            return Ok(None);
        };
        (job.frames.clone(), job.process.ended.take())
    };
    // The end of the frames.
    frames.lock().await.take();
    let encoded = ended(encoder).await;
    if encoded.killed || !video.is_running(id) {
        return Ok(None);
    }
    encoded.check()?;
    let (muxer, part, output) = {
        let mut slot = video.slot();
        let Some(job) = slot.as_mut().filter(|job| job.id == id) else {
            return Ok(None);
        };
        let (video, melody, part) = (job.video(), job.melody(), part_path(&job.output));
        let mut audio: Vec<Input> = recordings
            .iter()
            .map(|recording| Input {
                path: &recording.path,
                offset: recording.offset,
                volume: recording.volume,
            })
            .collect();
        if melody.is_file() {
            audio.push(Input {
                path: &melody,
                offset: 0.0,
                volume: 1.0,
            });
        }
        let args = mux_args(&video, &audio, start, seconds, &part);
        let (process, _) = Process::spawn(&job.ffmpeg, args, false)
            .map_err(|error| format!("ffmpeg did not start: {error}"))?;
        // The encoder, which has ended, is replaced.
        job.process = process;
        (job.process.ended.take(), part, job.output.clone())
    };
    let muxed = ended(muxer).await;
    if muxed.killed || !video.is_running(id) {
        return Ok(None);
    }
    muxed.check()?;
    tokio::fs::rename(&part, &output)
        .await
        .map_err(|error| format!("{}: {error}", output.display()))?;
    Ok(Some(output))
}

#[tauri::command]
pub async fn video_cancel(video: State<'_, Video>) -> Answer<bool> {
    Ok(video.cancel().await)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn the_video_is_named_after_the_score() {
        assert_eq!(
            suggested_name(Path::new("/songs/Song G.ly")),
            "Song G (karaoke).mp4"
        );
    }

    #[test]
    fn frames_come_as_jpegs_on_stdin_and_go_out_as_h264() {
        let args = strings(&encode_args(30, Path::new("/tmp/v/video.mp4")));
        assert_eq!(
            args[4..12],
            [
                "-f",
                "image2pipe",
                "-framerate",
                "30",
                "-c:v",
                "mjpeg",
                "-i",
                "-"
            ]
        );
        assert!(args.windows(2).any(|pair| pair == ["-pix_fmt", "yuv420p"]));
        assert_eq!(args.last().unwrap(), "/tmp/v/video.mp4");
    }

    /// A file in the score from its start, as it is.
    fn plain(path: &str) -> Input<'_> {
        Input {
            path: Path::new(path),
            offset: 0.0,
            volume: 1.0,
        }
    }

    #[test]
    fn a_backing_track_and_a_melody_are_summed_as_heard() {
        let video = Path::new("/t/video.mp4");
        let out = Path::new("/s/out.mp4");
        let none = strings(&mux_args(video, &[], 0.0, None, out));
        assert!(!none.contains(&"-c:a".to_string()));
        let one = strings(&mux_args(video, &[plain("/s/backing.wav")], 0.0, None, out));
        assert!(one.windows(2).any(|pair| pair == ["-map", "1:a"]));
        let both = [plain("/s/backing.wav"), plain("/t/melody.wav")];
        let two = strings(&mux_args(video, &both, 0.0, None, out));
        assert!(
            two.contains(&"[1:a][2:a]amix=inputs=2:duration=longest:normalize=0[a]".to_string())
        );
        assert!(two.windows(2).any(|pair| pair == ["-map", "[a]"]));
        assert_eq!(two[two.len() - 3..], ["-f", "mp4", "/s/out.mp4"]);
    }

    #[test]
    fn the_sound_starts_where_the_video_does() {
        let video = Path::new("/t/video.mp4");
        let out = Path::new("/s/out.mp4");
        let both = [plain("/s/backing.wav"), plain("/t/melody.wav")];
        let late = strings(&mux_args(video, &both, 31.88, None, out));
        let cut = "atrim=start=31.880,asetpts=PTS-STARTPTS";
        assert!(late.contains(&format!(
            "[1:a]{cut}[s1];[2:a]{cut}[s2];[s1][s2]amix=inputs=2:duration=longest:normalize=0[a]"
        )));
        let early = strings(&mux_args(video, &both[..1], -4.09, None, out));
        assert!(early.contains(&"[1:a]adelay=delays=4090:all=1[a]".to_string()));
        assert!(early.windows(2).any(|pair| pair == ["-map", "[a]"]));
    }

    #[test]
    fn the_audio_track_comes_in_at_its_offset_and_volume() {
        let video = Path::new("/t/video.mp4");
        let out = Path::new("/s/out.mp4");
        let song = |offset| Input {
            path: Path::new("/s/song.mp3"),
            offset,
            volume: 0.4,
        };
        // 2.5 s into the score, the video from 1 s: 1.5 s of silence first.
        let later = [song(2.5), plain("/t/melody.wav")];
        let args = strings(&mux_args(video, &later, 1.0, Some(9.5), out));
        assert!(args.contains(&"[1:a]adelay=delays=1500:all=1,volume=0.400[s1];[2:a]atrim=start=1.000,asetpts=PTS-STARTPTS[s2];[s1][s2]amix=inputs=2:duration=longest:normalize=0[a]".to_string()));
        assert!(args.windows(2).any(|pair| pair == ["-t", "9.500"]));
        // Already 1.25 s in at the score's start, the video from it: cut.
        let earlier = strings(&mux_args(video, &[song(-1.25)], 0.0, None, out));
        assert!(
            earlier.contains(
                &"[1:a]atrim=start=1.250,asetpts=PTS-STARTPTS,volume=0.400[a]".to_string()
            )
        );
        assert!(!earlier.contains(&"-t".to_string()));
    }

    #[test]
    fn the_last_step_writes_a_hidden_mp4_beside_the_output() {
        assert_eq!(
            part_path(Path::new("/songs/smoke (karaoke).mp4")),
            Path::new("/songs/.smoke (karaoke).part.mp4")
        );
    }

    #[test]
    fn what_ffmpeg_says_is_kept_to_its_last_lines() {
        let mut tail = Tail::default();
        tail.push(b"short\n");
        assert_eq!(tail.0, b"short\n");
        for line in 0..1000 {
            tail.push(format!("line {line:04} {}\n", "x".repeat(40)).as_bytes());
        }
        assert!(tail.0.len() <= TAIL);
        assert!(tail.0.starts_with(b"line "));
        assert!(
            tail.0
                .ends_with(format!("line 0999 {}\n", "x".repeat(40)).as_bytes())
        );
        // One line longer than the tail is cut where it must be.
        let mut tail = Tail::default();
        tail.push(&vec![b'y'; TAIL * 3]);
        assert_eq!(tail.0.len(), TAIL);
    }

    #[tokio::test]
    async fn stderr_is_read_to_its_end_and_its_tail_kept() {
        let said = vec![b'z'; TAIL * 10];
        assert_eq!(drain(&said[..]).await.len(), TAIL);
    }

    fn sh(script: &str, input: bool) -> (Process, Option<ChildStdin>) {
        Process::spawn(
            Path::new("/bin/sh"),
            vec!["-c".into(), script.into()],
            input,
        )
        .expect("sh starts")
    }

    const SECONDS: std::time::Duration = std::time::Duration::from_secs(20);

    #[tokio::test]
    async fn an_ffmpeg_that_says_a_lot_still_takes_its_frames() {
        // 1 MB on stderr, far past a pipe's buffer, before reading stdin.
        let (mut process, stdin) = sh(
            "head -c 1048576 /dev/zero | tr '\\0' x >&2; cat > /dev/null; echo the end >&2",
            true,
        );
        let mut stdin = stdin.unwrap();
        let frames = vec![0u8; 4 * 1024 * 1024];
        tokio::time::timeout(SECONDS, stdin.write_all(&frames))
            .await
            .expect("the frames are taken")
            .unwrap();
        drop(stdin);
        let ended = tokio::time::timeout(SECONDS, ended(process.ended.take()))
            .await
            .expect("it ends");
        assert!(ended.check().is_ok() && !ended.killed);
        assert!(ended.said.len() <= TAIL);
        assert!(ended.said.ends_with(b"the end\n"));
    }

    #[tokio::test]
    async fn a_kill_ends_a_write_that_ffmpeg_is_not_reading() {
        let (mut process, stdin) = sh("sleep 60", true);
        let mut stdin = stdin.unwrap();
        let writing =
            tokio::spawn(async move { stdin.write_all(&vec![0u8; 4 * 1024 * 1024]).await });
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert!(!writing.is_finished(), "the write is blocked");
        tokio::time::timeout(SECONDS, process.kill())
            .await
            .expect("the kill does not wait for the write");
        let written = tokio::time::timeout(SECONDS, writing)
            .await
            .expect("the write ends")
            .unwrap();
        assert!(written.is_err());
        let ended = ended(process.ended.take()).await;
        assert!(ended.killed);
    }

    #[tokio::test]
    async fn ffmpeg_failing_says_why() {
        let (mut process, _) = sh("echo 'Invalid data found' >&2; exit 1", false);
        let ended = ended(process.ended.take()).await;
        assert_eq!(
            ended.check().unwrap_err(),
            "ffmpeg failed: Invalid data found"
        );
    }

    fn job(id: u64, folder: &Path) -> (Job, oneshot::Receiver<Ended>) {
        let (mut process, stdin) = sh("sleep 60", true);
        let ended = process.ended.take().unwrap();
        let dir = folder.join(format!("temp-{id}"));
        std::fs::create_dir_all(&dir).unwrap();
        let job = Job {
            id,
            ffmpeg: PathBuf::from("/bin/sh"),
            process,
            frames: Arc::new(Mutex::new(stdin)),
            dir,
            output: folder.join(format!("song {id} (karaoke).mp4")),
        };
        (job, ended)
    }

    #[tokio::test]
    async fn a_new_export_discards_one_the_page_lost_track_of() {
        let folder = tempfile::tempdir().unwrap();
        let video = Video::default();
        let (lost, lost_ended) = job(1, folder.path());
        let lost_dir = lost.dir.clone();
        video.begin(lost).await;
        let (next, _next_ended) = job(2, folder.path());
        tokio::time::timeout(SECONDS, video.begin(next))
            .await
            .expect("the lost one is discarded");
        assert!(lost_ended.await.unwrap().killed);
        assert!(!lost_dir.exists());
        assert!(video.is_running(2) && !video.is_running(1));
        assert!(video.cancel().await);
        assert!(!video.cancel().await);
    }

    #[tokio::test]
    async fn a_cancel_in_the_last_step_kills_it_and_deletes_its_part_file() {
        let folder = tempfile::tempdir().unwrap();
        let video = Video::default();
        let (job, ended) = job(3, folder.path());
        let part = part_path(&job.output);
        std::fs::write(&part, b"half an mp4").unwrap();
        std::fs::write(&job.output, b"the file that had the name").unwrap();
        let output = job.output.clone();
        video.begin(job).await;
        assert!(tokio::time::timeout(SECONDS, video.cancel()).await.unwrap());
        assert!(ended.await.unwrap().killed);
        assert!(!part.exists());
        assert_eq!(
            std::fs::read(&output).unwrap(),
            b"the file that had the name"
        );
    }
}
