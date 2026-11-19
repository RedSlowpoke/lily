//! The reader of Guitar Pro 6's `.gpx` (D166, README §3): BCFZ's bitwise
//! decompression and the BCFS sector file system it unpacks to
//! (`container`), and the `score.gpif` XML in it (`gpif`), whose bars,
//! voices, beats, notes and rhythms refer to one another by id.
//!
//! Drum tracks are read with their beats, but their notes are left out:
//! GPX names a drum by an element of Guitar Pro 6's kit, and the table from
//! those to General MIDI is still to be written (D166, README §3.8). Each
//! beat that had drum notes becomes a rest of its length.
mod container;
mod gpif;

use crate::{Error, Song};

/// Reads a `.gpx` file, compressed (`BCFZ`) or not (`BCFS`).
pub fn read(bytes: &[u8]) -> Result<Song, Error> {
    let image = container::image(bytes)?;
    let score = container::file(&image, "score.gpif")?;
    let xml = std::str::from_utf8(&score)
        .map_err(|_| Error::Container("its score.gpif is not UTF-8".into()))?;
    gpif::parse(xml)
}

#[cfg(test)]
mod tests {
    use super::container::tests::{compress, sample_image};
    use super::gpif::tests::FIXTURE;
    use super::*;

    #[test]
    fn reads_a_score_through_both_containers() {
        let image = sample_image(FIXTURE.as_bytes()).bytes();
        let plain = read(&image).unwrap();
        assert_eq!(plain.info.title, "Song");
        assert_eq!(read(&compress(&image)).unwrap(), plain);
        assert_eq!(crate::read(&compress(&image), Some("gpx")).unwrap(), plain);
    }

    #[test]
    fn a_score_that_is_not_text_is_an_error() {
        let image = sample_image(&[0xff, 0xfe, 0]).bytes();
        assert!(matches!(read(&image), Err(Error::Container(_))));
    }
}
