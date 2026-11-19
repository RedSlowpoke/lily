//! Scores the system opens with Lily Studio (DECISIONS D123): a double click
//! in Finder, Open With, a file dropped on the Dock icon, which macOS sends as
//! `RunEvent::Opened`; elsewhere, and from a terminal, the paths on the
//! command line, which a second launch hands to the first (lib.rs). Each
//! opens as Open Score's choice does, so its folder becomes the open folder.
//! One that comes before the page listens, as the file that launched the app
//! does, waits until it subscribes.
//!
//! And the export the studio opens elsewhere (D162 stage 3): the `.mid` of
//! Export for GarageBand in GarageBand, when Launch Services finds it, and
//! otherwise its folder in the Finder.
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use lily_engrave::files::is_score_file;
use lily_engrave::paths::resolve_from;
use lily_session::{Studio, StudioEvent, lock};
use tauri::{AppHandle, Manager, Url};
use tauri_plugin_opener::OpenerExt;

/// The scores that came before the page listened; None once it does.
pub struct Opening(Mutex<Option<Vec<PathBuf>>>);

impl Default for Opening {
    fn default() -> Opening {
        Opening(Mutex::new(Some(Vec::new())))
    }
}

impl Opening {
    /// `files` to open now, or none when they wait for the page.
    fn arrive(&self, files: Vec<PathBuf>) -> Vec<PathBuf> {
        match lock(&self.0).as_mut() {
            Some(waiting) => {
                waiting.extend(files);
                Vec::new()
            }
            None => files,
        }
    }

    /// The page listens from now on: what waited, to open now.
    fn listen(&self) -> Vec<PathBuf> {
        lock(&self.0).take().unwrap_or_default()
    }
}

/// The files of the URLs macOS opens the app with.
#[cfg(any(target_os = "macos", test))]
pub fn from_urls(urls: &[Url]) -> Vec<PathBuf> {
    urls.iter()
        .filter(|url| url.scheme() == "file")
        .filter_map(|url| url.to_file_path().ok())
        .collect()
}

/// The paths among a launch's arguments, without the program, against its working directory.
pub fn from_args(args: impl IntoIterator<Item = String>, cwd: &Path) -> Vec<PathBuf> {
    args.into_iter()
        .skip(1)
        .filter(|arg| !arg.starts_with('-'))
        .map(|arg| resolve_from(cwd, arg))
        .collect()
}

/// Opens the scores among `files`, now or once the page listens; anything else is left alone.
pub fn open(app: &AppHandle, files: Vec<PathBuf>) {
    let scores = files
        .into_iter()
        .filter(|file| is_score_file(file) && file.is_file())
        .collect();
    let now = app.state::<Opening>().arrive(scores);
    // The window of a launch shows itself once the page has loaded (lib.rs).
    if !now.is_empty() {
        crate::focus(app);
    }
    show(app, now);
}

/// The page has subscribed (commands::subscribe): what waited opens.
pub fn listening(app: &AppHandle) {
    let waited = app.state::<Opening>().listen();
    show(app, waited);
}

/// Opens each file in turn, as Open Score does: the page shows the last.
fn show(app: &AppHandle, files: Vec<PathBuf>) {
    if files.is_empty() {
        return;
    }
    let studio = app.state::<Arc<Studio>>().inner().clone();
    studio.clone().spawn(async move {
        for file in files {
            let opened = studio.open_file_at(&file).await;
            studio.send(StudioEvent::Opened { opened });
        }
    });
}

/// GarageBand's bundle id, which Launch Services looks up (D162 stage 3).
const GARAGEBAND: &str = "com.apple.garageband10";

/// How an export for GarageBand is shown once written (D162 stage 3).
#[derive(Debug, PartialEq, Eq)]
pub enum ShowExport {
    /// The `.mid` opened with GarageBand, which imports it as a new project.
    GarageBand(PathBuf),
    /// The folder selected in a Finder window.
    Finder(PathBuf),
}

/// GarageBand when Launch Services found it (`garageband`, the app's
/// bundle), which opens `midi`; otherwise the Finder, which shows `folder`.
pub fn show_export(garageband: Option<&Path>, midi: &Path, folder: &Path) -> ShowExport {
    match garageband {
        Some(_) => ShowExport::GarageBand(midi.to_path_buf()),
        None => ShowExport::Finder(folder.to_path_buf()),
    }
}

/// The arguments of `/usr/bin/open` that open `midi` with GarageBand by its
/// bundle id: Launch Services, no Apple Events (D162 stage 3).
fn open_with_garageband(midi: &Path) -> Vec<std::ffi::OsString> {
    vec!["-b".into(), GARAGEBAND.into(), midi.into()]
}

/// Where Launch Services has GarageBand, if anywhere.
#[cfg(target_os = "macos")]
fn find_garageband() -> Option<PathBuf> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;
    NSWorkspace::sharedWorkspace()
        .URLForApplicationWithBundleIdentifier(&NSString::from_str(GARAGEBAND))
        .and_then(|url| url.path())
        .map(|path| PathBuf::from(path.to_string()))
}

#[cfg(not(target_os = "macos"))]
fn find_garageband() -> Option<PathBuf> {
    None
}

/// Shows the export written in `folder` (D162 stage 3): opens its `midi` in
/// GarageBand, or reveals `folder` in the Finder when GarageBand is not
/// found or would not open it. The export is written either way, so a
/// failure here is only what it shows. Blocks while `open` asks Launch
/// Services.
pub fn show_garageband_export(app: &AppHandle, midi: &Path, folder: &Path) -> ShowExport {
    let shown = show_export(find_garageband().as_deref(), midi, folder);
    if let ShowExport::GarageBand(midi) = &shown {
        let opened = std::process::Command::new("/usr/bin/open")
            .args(open_with_garageband(midi))
            .status()
            .is_ok_and(|status| status.success());
        if opened {
            return shown;
        }
    }
    let _ = app.opener().reveal_item_in_dir(folder);
    ShowExport::Finder(folder.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_wait_until_the_page_listens() {
        let opening = Opening::default();
        assert!(opening.arrive(vec!["/a.ly".into()]).is_empty());
        assert!(opening.arrive(vec!["/b.ly".into()]).is_empty());
        assert_eq!(
            opening.listen(),
            [PathBuf::from("/a.ly"), PathBuf::from("/b.ly")]
        );
        assert_eq!(
            opening.arrive(vec!["/c.ly".into()]),
            [PathBuf::from("/c.ly")]
        );
        // A page loaded again subscribes again; nothing waits for it.
        assert!(opening.listen().is_empty());
    }

    #[test]
    fn arguments_are_paths_from_the_working_directory() {
        let args = ["lily-studio", "--smoke-test", "song.ly", "/scores/a.ily"].map(String::from);
        assert_eq!(
            from_args(args, Path::new("/Users/me")),
            [
                PathBuf::from("/Users/me/song.ly"),
                PathBuf::from("/scores/a.ily")
            ]
        );
    }

    #[test]
    fn only_file_urls_are_files() {
        let urls = [
            Url::parse("file:///Users/me/My%20Song.ly").unwrap(),
            Url::parse("https://lilypond.org/a.ly").unwrap(),
        ];
        assert_eq!(from_urls(&urls), [PathBuf::from("/Users/me/My Song.ly")]);
    }

    #[test]
    fn an_export_opens_in_garageband_when_it_is_found_and_otherwise_in_the_finder() {
        let folder = Path::new("/Users/me/Song (GarageBand)");
        let midi = folder.join("Song.mid");
        assert_eq!(
            show_export(
                Some(Path::new("/Applications/GarageBand.app")),
                &midi,
                folder
            ),
            ShowExport::GarageBand(midi.clone())
        );
        assert_eq!(
            show_export(None, &midi, folder),
            ShowExport::Finder(folder.to_path_buf())
        );
        assert_eq!(
            open_with_garageband(&midi),
            [
                "-b",
                "com.apple.garageband10",
                "/Users/me/Song (GarageBand)/Song.mid"
            ]
            .map(std::ffi::OsString::from)
        );
    }
}
