//! File › Import Guitar Pro… (DECISIONS D166) once the user has chosen: a
//! `.gp3`, `.gp4`, `.gp5` or `.gpx` file's bytes read and written as a
//! LilyPond score by `lily-guitarpro`, the score saved where the user said
//! and opened as New Score's is. The Mac app reads the file the open panel
//! chose or the window was given, and the web's worker the file the page
//! uploaded (D55); both convert before asking where to save, so a file that
//! cannot be imported is refused first. The source file is only read, never
//! let into what the page may touch.
use std::path::{Path, PathBuf};

use lily_engrave::files;
use lily_guitarpro::{Error, lilypond};

use crate::ops::Opened;
use crate::studio::{Studio, lock};

type Answer<T> = Result<T, String>;

/// What Import Guitar Pro… offers, without the dot.
pub const EXTENSIONS: &[&str] = &["gp3", "gp4", "gp5", "gpx"];
/// Guitar Pro's other extensions: read only to say why they are refused
/// (`.gp` of GP7 and GP8, `.gtp` of GP1 and GP2).
const REFUSED_EXTENSIONS: &[&str] = &["gp", "gtp"];
/// A Guitar Pro file is a few hundred kilobytes; a file far larger is not one.
pub const MAX_SIZE: u64 = 32 * 1024 * 1024;

const IMPORTED: &str = "Lily Studio imports .gp3, .gp4, .gp5 and .gpx";

/// The extensions of the open panel, as `ops::score_extensions` gives a score's.
pub fn extensions() -> Vec<String> {
    EXTENSIONS.iter().map(|e| e.to_string()).collect()
}

/// True for a name with one of Guitar Pro's extensions, imported or refused,
/// in any case: what a drop on the window may hand to `read_source`.
pub fn is_guitar_pro_file(file: impl AsRef<Path>) -> bool {
    let extension = file
        .as_ref()
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    extension.is_some_and(|e| {
        EXTENSIONS.contains(&e.as_str()) || REFUSED_EXTENSIONS.contains(&e.as_str())
    })
}

/// The score's name for `source`: `song.gp5` gives `song.ly`.
pub fn score_name(source: impl AsRef<Path>) -> String {
    let source = source.as_ref();
    let stem = source
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "Untitled".to_string());
    format!("{stem}.ly")
}

/// `chosen` as the score's path: as it is with a score extension, else with
/// `.ly` after it, so `Song v1.2` becomes `Song v1.2.ly`.
pub fn score_path(chosen: &Path) -> PathBuf {
    if files::is_score_file(chosen) {
        chosen.to_path_buf()
    } else {
        let mut name = chosen.as_os_str().to_os_string();
        name.push(".ly");
        PathBuf::from(name)
    }
}

/// The bytes of `source`, a file the user chose or dropped, refusing what is
/// no Guitar Pro file by its name or too large to be one.
pub async fn read_source(source: &Path) -> Answer<Vec<u8>> {
    let name = file_name(source);
    if !is_guitar_pro_file(source) {
        return Err(not_guitar_pro(&name));
    }
    let meta = tokio::fs::metadata(source)
        .await
        .map_err(|error| format!("{name} could not be read: {error}"))?;
    if !meta.is_file() {
        return Err(not_guitar_pro(&name));
    }
    if meta.len() > MAX_SIZE {
        return Err(too_large(&name));
    }
    tokio::fs::read(source)
        .await
        .map_err(|error| format!("{name} could not be read: {error}"))
}

/// The LilyPond score of a Guitar Pro file named `name`, from its `bytes`,
/// or a message that names the file and says why it cannot be imported.
pub fn convert(name: &str, bytes: &[u8]) -> Answer<String> {
    if bytes.len() as u64 > MAX_SIZE {
        return Err(too_large(name));
    }
    let hint = Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().into_owned());
    // The format's own refusal (GP7's .gp, GP1 and GP2) says what the file
    // is; a reader's says what in it could not be read.
    if let Err(error) = lily_guitarpro::sniff(bytes, hint.as_deref()) {
        return Err(refused(name, &error));
    }
    let song = lily_guitarpro::read(bytes, hint.as_deref()).map_err(|error| {
        format!(
            "{name} could not be read: {}",
            sentence_inside(&error.to_string())
        )
    })?;
    let text = lilypond::write(&song);
    if text.trim().is_empty() {
        return Err(format!(
            "{name} could not be imported: no score could be written from it."
        ));
    }
    Ok(text)
}

/// Why `sniff` refused the file `name`.
fn refused(name: &str, error: &Error) -> String {
    match error {
        Error::Unsupported(what) => format!("{name} is {}; {IMPORTED}.", sentence_inside(what)),
        Error::UnknownFormat { .. } => not_guitar_pro(name),
        other => format!(
            "{name} could not be read: {}",
            sentence_inside(&other.to_string())
        ),
    }
}

fn not_guitar_pro(name: &str) -> String {
    format!("{name} is not a Guitar Pro file; {IMPORTED}.")
}

fn too_large(name: &str) -> String {
    format!(
        "{name} is too large for a Guitar Pro file ({} MB at most).",
        MAX_SIZE / 1024 / 1024
    )
}

/// A sentence of the crate's, after a colon or inside another: its first
/// letter in lower case unless it starts a name (`GP`, `BCFZ`).
fn sentence_inside(text: &str) -> String {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(first), second)
            if first.is_uppercase() && !second.is_some_and(char::is_uppercase) =>
        {
            first
                .to_lowercase()
                .chain(text[first.len_utf8()..].chars())
                .collect()
        }
        _ => text.to_string(),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

impl Studio {
    /// Where Import Guitar Pro…'s save panel starts: `<source>.ly` in the
    /// open folder, else beside `source`.
    pub fn guitar_pro_suggestion(&self, source: &Path) -> PathBuf {
        let folder = lock(&self.access).folder.clone();
        let folder = folder
            .or_else(|| source.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| self.documents.clone());
        folder.join(score_name(source))
    }

    /// Imports the Guitar Pro file `name`, whose `bytes` were read or
    /// uploaded, as a score at `chosen` (`.ly` added when it has no score
    /// extension), and opens it as `create_score` does. A file there is
    /// replaced: the save panel, or the page, has asked.
    pub async fn import_guitar_pro(
        &self,
        name: &str,
        bytes: &[u8],
        chosen: &Path,
    ) -> Answer<Opened> {
        let text = convert(name, bytes)?;
        self.create_imported_score(chosen, &text).await
    }

    /// Writes `text`, a score `convert` made, at `chosen` (`.ly` added when
    /// it has no score extension), and opens it as `create_score` does.
    pub async fn create_imported_score(&self, chosen: &Path, text: &str) -> Answer<Opened> {
        let file = score_path(chosen);
        tokio::fs::write(&file, text)
            .await
            .map_err(|error| format!("{} could not be written: {error}", file_name(&file)))?;
        Ok(self.open_created(&file).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::{Paths, StudioOptions};
    use std::sync::Arc;

    fn header(text: &str) -> Vec<u8> {
        let mut bytes = vec![text.len() as u8];
        bytes.extend_from_slice(text.as_bytes());
        bytes.resize(31, 0);
        bytes
    }

    async fn studio(dir: &Path) -> Arc<Studio> {
        let data = dir.join("data");
        std::fs::create_dir_all(&data).unwrap();
        Studio::new(StudioOptions::new(Paths {
            data,
            documents: dir.join("Documents"),
            runtime: dir.join("runtime"),
        }))
        .await
    }

    #[test]
    fn names_and_paths() {
        assert_eq!(score_name("/Music/band-song.gp5"), "band-song.ly");
        assert_eq!(score_name("/Music/Song.GPX"), "Song.ly");
        assert_eq!(score_path(Path::new("/s/Song")), Path::new("/s/Song.ly"));
        assert_eq!(
            score_path(Path::new("/s/Song v1.2")),
            Path::new("/s/Song v1.2.ly")
        );
        assert_eq!(score_path(Path::new("/s/Song.ly")), Path::new("/s/Song.ly"));
        assert_eq!(
            score_path(Path::new("/s/parts.ily")),
            Path::new("/s/parts.ily")
        );
        assert!(is_guitar_pro_file("/a/b.GP5"));
        assert!(is_guitar_pro_file("/a/b.gp"));
        assert!(!is_guitar_pro_file("/a/b.mid"));
        assert!(!is_guitar_pro_file("/a/gp5"));
    }

    #[test]
    fn refuses_what_is_not_imported_and_says_why() {
        assert_eq!(
            convert("song.mid", b"MThd\0\0\0\x06"),
            Err(
                "song.mid is not a Guitar Pro file; Lily Studio imports .gp3, .gp4, .gp5 and .gpx."
                    .into()
            )
        );
        assert_eq!(
            convert("song.gp", b"PK\x03\x04rest"),
            Err("song.gp is a Guitar Pro 7 or 8 file (.gp); Lily Studio imports .gp3, .gp4, .gp5 and .gpx.".into())
        );
        assert_eq!(
            convert("old.gp5", &header("FICHIER GUITAR PRO v2.21")),
            Err(
                "old.gp5 is a Guitar Pro 2 file; Lily Studio imports .gp3, .gp4, .gp5 and .gpx."
                    .into()
            )
        );
        // A file the format admits, which its reader refuses, names the reader's reason.
        let refused = convert("short.gp5", &header("FICHIER GUITAR PRO v5.10")).unwrap_err();
        assert!(
            refused.starts_with("short.gp5 could not be read"),
            "{refused}"
        );
    }

    #[test]
    fn a_sentence_inside_another() {
        assert_eq!(
            sentence_inside("The file ends early."),
            "the file ends early."
        );
        assert_eq!(
            sentence_inside("A Guitar Pro 7 file"),
            "a Guitar Pro 7 file"
        );
        assert_eq!(sentence_inside("GPIF is broken"), "GPIF is broken");
        assert_eq!(sentence_inside(""), "");
    }

    #[tokio::test]
    async fn writes_the_score_with_ly_appended_and_opens_it() {
        let dir = tempfile::tempdir().unwrap();
        let studio = studio(dir.path()).await;
        let songs = dir.path().join("Songs");
        std::fs::create_dir_all(&songs).unwrap();
        let text = "\\version \"2.24.0\"\n{ c'4 }\n";
        let opened = studio
            .create_imported_score(&songs.join("Tune"), text)
            .await
            .unwrap();
        let file = paths_resolved(&songs.join("Tune.ly"));
        assert_eq!(opened.file.as_deref(), Some(file.as_path()));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
        // Outside any open folder: its own folder opens, and the page may use the score.
        assert_eq!(opened.listing.folder, paths_resolved(&songs));
        assert!(
            studio
                .check(&serde_json::json!(file.to_string_lossy()))
                .is_ok()
        );
        // Replaced when chosen again: the save panel asked.
        studio
            .create_imported_score(&songs.join("Tune.ly"), "% again\n")
            .await
            .unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "% again\n");
        studio.dispose().await;
    }

    #[tokio::test]
    async fn an_import_into_the_open_folder_keeps_it_open() {
        let dir = tempfile::tempdir().unwrap();
        let studio = studio(dir.path()).await;
        let scores = dir.path().join("Scores");
        std::fs::create_dir_all(scores.join("Tabs")).unwrap();
        studio.open_folder_at(&scores, None).await;
        let opened = studio
            .create_imported_score(&scores.join("Tabs").join("song.ly"), "{ c'4 }\n")
            .await
            .unwrap();
        assert_eq!(opened.listing.folder, paths_resolved(&scores));
        assert_eq!(
            studio.guitar_pro_suggestion(Path::new("/Music/Song.gpx")),
            paths_resolved(&scores).join("Song.ly")
        );
        studio.dispose().await;
    }

    #[tokio::test]
    async fn a_file_that_is_not_guitar_pro_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let studio = studio(dir.path()).await;
        let chosen = dir.path().join("notes");
        let refused = studio
            .import_guitar_pro("notes.txt", b"just some text", &chosen)
            .await
            .err()
            .expect("refused");
        assert_eq!(
            refused,
            "notes.txt is not a Guitar Pro file; Lily Studio imports .gp3, .gp4, .gp5 and .gpx."
        );
        assert!(!dir.path().join("notes.ly").exists());
        assert!(!chosen.exists());
        // Before reading, by its name, for a file dropped on the window.
        let text = dir.path().join("notes.txt");
        std::fs::write(&text, "just some text").unwrap();
        assert!(
            read_source(&text)
                .await
                .unwrap_err()
                .contains("is not a Guitar Pro file")
        );
        studio.dispose().await;
    }

    /// The samples of D166, copyrighted songs that stay out of the
    /// repository, imported through the session as the menu does: read from
    /// `GUITAR_PRO_SAMPLES` (unset, none is found), each one
    /// not found skipped and named.
    #[tokio::test]
    async fn the_samples_import() {
        const SAMPLES: [&str; 7] = [
            "gpx-a.gpx",
            "gp4-a.gp4",
            "gp3-b.gp3",
            "gp3-a.gp3",
            "gp5-b.gp5",
            "gp5-c.gp5",
            "gp5-a.gp5",
        ];
        let samples = std::env::var_os("GUITAR_PRO_SAMPLES")
            .map(PathBuf::from)
            .unwrap_or_default();
        let dir = tempfile::tempdir().unwrap();
        let studio = studio(dir.path()).await;
        let mut imported = 0;
        for name in SAMPLES {
            let source = samples.join(name);
            if !source.is_file() {
                eprintln!("skipped {}: not found", source.display());
                continue;
            }
            let bytes = read_source(&source).await.unwrap();
            // As the save panel gives it: no extension, so `.ly` is added.
            let chosen = dir.path().join(name.rsplit_once('.').unwrap().0);
            let opened = studio
                .import_guitar_pro(name, &bytes, &chosen)
                .await
                .unwrap_or_else(|error| panic!("{error}"));
            let file = opened.file.unwrap();
            assert_eq!(file.extension().unwrap(), "ly", "{name}");
            let text = std::fs::read_to_string(&file).unwrap();
            assert!(text.starts_with("\\version"), "{name}");
            assert!(text.contains("\\new TabStaff"), "{name}");
            assert!(text.contains("\\midi"), "{name}");
            imported += 1;
        }
        studio.dispose().await;
        eprintln!(
            "imported {imported} of {} samples from {}",
            SAMPLES.len(),
            samples.display()
        );
    }

    fn paths_resolved(path: &Path) -> PathBuf {
        lily_engrave::paths::resolve(path)
    }
}
