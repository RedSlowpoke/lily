//! Guitar Pro import (DECISIONS D166): a `.gp3`, `.gp4`, `.gp5` or `.gpx`
//! read into one song model (`model`), and that model written as a LilyPond
//! score (`lilypond`), each guitar or bass a Staff and a TabStaff in its own
//! tuning. `read` tells the formats apart by their first bytes and hands the
//! file to `gp345` (the binary GP3–GP5) or `gpx` (Guitar Pro 6's container
//! and its `score.gpif`). Both readers are ours, written from
//! `docs/research/guitar-pro/README.md` and the samples, never from another
//! project's code, so the crate stays MIT as the studio is. The session
//! crate runs it for File › Import Guitar Pro… in the app and on the web
//! server (D55); it depends on neither Tauri nor the file system.
use std::fmt;

pub mod gp345;
pub mod gpx;
pub mod lilypond;
pub mod model;

pub use model::*;

/// The magic GP3–GP5 files start with, after a length byte (README §2.2).
const BINARY_MAGIC: &[u8] = b"FICHIER GUITAR PRO v";

/// Which reader a file is for, from its first bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// GP3, GP4 or GP5: `FICHIER GUITAR PRO v3.00`, `v4.06`, `v5.10`, ….
    Binary(Version),
    /// Guitar Pro 6: a BCFZ (compressed) or BCFS container holding
    /// `score.gpif` (README §3).
    Gpx,
}

/// A binary file's version, `v5.10` being `{ major: 5, minor: 10 }`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02}", self.major, self.minor)
    }
}

/// Why a file could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not a Guitar Pro file this crate knows; `hint` is the extension it
    /// was given, if any.
    UnknownFormat { hint: Option<String> },
    /// A Guitar Pro file of a kind not read (yet): GP1, GP2, GP7's `.gp`,
    /// or a reader still to be written.
    Unsupported(String),
    /// The file ends before a value at `offset` that needs `needed` bytes.
    Truncated { offset: usize, needed: usize },
    /// A value at `offset` that cannot be: a negative count, an unknown
    /// duration, a string longer than the file.
    Invalid { offset: usize, what: String },
    /// The `.gpx` container or its XML is broken.
    Container(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnknownFormat { hint: Some(ext) } => {
                write!(
                    f,
                    "This .{ext} file is not a Guitar Pro file Lily Studio can read."
                )
            }
            Error::UnknownFormat { hint: None } => {
                write!(f, "This is not a Guitar Pro file Lily Studio can read.")
            }
            Error::Unsupported(what) => write!(f, "{what} cannot be imported yet."),
            Error::Truncated { offset, needed } => {
                write!(
                    f,
                    "The file ends early: {needed} more bytes were needed at byte {offset}."
                )
            }
            Error::Invalid { offset, what } => {
                write!(f, "The file is damaged at byte {offset}: {what}.")
            }
            Error::Container(what) => write!(f, "The Guitar Pro 6 file is damaged: {what}."),
        }
    }
}

impl std::error::Error for Error {}

/// Which reader the file is for, from its first bytes alone.
/// `extension_hint` (`"gp5"`, with or without the dot, any case) only
/// sharpens the error: the bytes decide.
pub fn sniff(bytes: &[u8], extension_hint: Option<&str>) -> Result<Format, Error> {
    if bytes.starts_with(b"BCFZ") || bytes.starts_with(b"BCFS") {
        return Ok(Format::Gpx);
    }
    if let Some(version) = binary_version(bytes) {
        return match version.major {
            3..=5 => Ok(Format::Binary(version)),
            _ => Err(Error::Unsupported(format!(
                "A Guitar Pro {} file",
                version.major
            ))),
        };
    }
    if bytes.starts_with(b"FICHIER GUITARE PRO")
        || bytes
            .get(1..)
            .is_some_and(|b| b.starts_with(b"FICHIER GUITARE PRO"))
    {
        return Err(Error::Unsupported("A Guitar Pro 1 or 2 file".into()));
    }
    if bytes.starts_with(b"PK\x03\x04") {
        return Err(Error::Unsupported("A Guitar Pro 7 or 8 file (.gp)".into()));
    }
    let hint = extension_hint
        .map(|e| e.trim_start_matches('.').to_ascii_lowercase())
        .filter(|e| !e.is_empty());
    Err(Error::UnknownFormat { hint })
}

/// `FICHIER GUITAR PRO vM.mm` after its length byte, as GP3–GP5 begin.
fn binary_version(bytes: &[u8]) -> Option<Version> {
    let len = usize::from(*bytes.first()?);
    let text = bytes.get(1..1 + len)?;
    let rest = text.strip_prefix(BINARY_MAGIC)?;
    let (major, minor) = std::str::from_utf8(rest).ok()?.split_once('.')?;
    let minor: String = minor.chars().take_while(char::is_ascii_digit).collect();
    Some(Version {
        major: major.parse().ok()?,
        minor: minor.parse().ok()?,
    })
}

/// Reads a Guitar Pro file of any format this crate knows.
pub fn read(bytes: &[u8], extension_hint: Option<&str>) -> Result<Song, Error> {
    match sniff(bytes, extension_hint)? {
        Format::Binary(version) => gp345::read(bytes, version),
        Format::Gpx => gpx::read(bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(text: &str) -> Vec<u8> {
        let mut bytes = vec![text.len() as u8];
        bytes.extend_from_slice(text.as_bytes());
        bytes.resize(31, 0);
        bytes
    }

    #[test]
    fn sniffs_each_binary_version() {
        for (text, major, minor) in [
            ("FICHIER GUITAR PRO v3.00", 3, 0),
            ("FICHIER GUITAR PRO v4.06", 4, 6),
            ("FICHIER GUITAR PRO v5.00", 5, 0),
            ("FICHIER GUITAR PRO v5.10", 5, 10),
        ] {
            assert_eq!(
                sniff(&header(text), None),
                Ok(Format::Binary(Version { major, minor })),
                "{text}"
            );
        }
    }

    #[test]
    fn sniffs_gpx_containers() {
        assert_eq!(sniff(b"BCFZ\x10\0\0\0", None), Ok(Format::Gpx));
        assert_eq!(sniff(b"BCFS\0\0\0\0", Some("gpx")), Ok(Format::Gpx));
    }

    #[test]
    fn refuses_what_it_does_not_read() {
        assert!(matches!(
            sniff(&header("FICHIER GUITAR PRO v2.21"), None),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(
            sniff(b"PK\x03\x04rest", Some("gp")),
            Err(Error::Unsupported(_))
        ));
        assert_eq!(
            sniff(b"MThd", Some(".GP5")),
            Err(Error::UnknownFormat {
                hint: Some("gp5".into())
            })
        );
        assert_eq!(sniff(b"", None), Err(Error::UnknownFormat { hint: None }));
    }
}
