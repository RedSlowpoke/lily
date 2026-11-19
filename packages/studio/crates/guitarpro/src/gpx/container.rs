//! A `.gpx`'s container (README §3.1, §3.2): BCFZ, a bit stream of literals
//! and copies that unpacks to a BCFS image, and BCFS, sectors of 4 KiB
//! holding a small file system. Its files are found by walking the
//! directories from the root at sector 1, never by scanning the sectors for
//! file entries: the root's own data begins with `2`, the kind of a file,
//! and a scan takes it for one.
//!
//! Every read is checked. A damaged file is an `Error`, never a panic, and
//! nothing grows beyond what the file declares: the unpacked image stops at
//! its declared size, which is itself capped, and a file's content at its
//! entry's size.
use std::collections::HashSet;

use crate::Error;

/// A BCFS sector.
const SECTOR: usize = 0x1000;
/// The largest image this reader unpacks. Guitar Pro 6 writes a few hundred
/// KiB for a long song; a declared size past this is a damaged header.
pub const MAX_IMAGE: usize = 64 << 20;
/// How deep directories may nest; the sample has only the root.
const MAX_DEPTH: usize = 16;
/// Where an entry's fields are in its sector (README §3.2).
const NAME: usize = 0x04;
const NAME_LEN: usize = 127;
const SIZE: usize = 0x8C;
const BLOCKS: usize = 0x94;
const KIND_DIRECTORY: i32 = 1;
const KIND_FILE: i32 = 2;

fn damaged(what: impl Into<String>) -> Error {
    Error::Container(what.into())
}

/// The BCFS image of a `.gpx`: unpacked when it is `BCFZ`, as it is when
/// `BCFS`.
pub fn image(bytes: &[u8]) -> Result<std::borrow::Cow<'_, [u8]>, Error> {
    if bytes.starts_with(b"BCFZ") {
        let image = unpack(bytes)?;
        if !image.starts_with(b"BCFS") {
            return Err(damaged(
                "its compressed data does not unpack to a BCFS image",
            ));
        }
        Ok(image.into())
    } else if bytes.starts_with(b"BCFS") {
        Ok(bytes.into())
    } else {
        Err(damaged("it begins with neither BCFZ nor BCFS"))
    }
}

/// Bits read from each byte's highest down (README §3.1).
struct Bits<'a> {
    bytes: &'a [u8],
    /// The next bit, counted from the first byte's highest.
    at: usize,
}

impl Bits<'_> {
    fn bit(&mut self) -> Option<u32> {
        let byte = *self.bytes.get(self.at / 8)?;
        let bit = (byte >> (7 - self.at % 8)) & 1;
        self.at += 1;
        Some(u32::from(bit))
    }

    /// `n` bits, the first read the highest.
    fn high_first(&mut self, n: u32) -> Option<u32> {
        (0..n).try_fold(0, |value, _| Some((value << 1) | self.bit()?))
    }

    /// `n` bits, the first read the lowest.
    fn low_first(&mut self, n: u32) -> Option<u32> {
        (0..n).try_fold(0, |value, i| Some(value | (self.bit()? << i)))
    }
}

/// Unpacks BCFZ: `BCFZ`, the unpacked size as a little-endian `int`, then
/// chunks until the output has that size. A chunk is a 0 bit and a literal
/// (a 2-bit count, low bit first, of bytes, high bit first), or a 1 bit and
/// a copy (a 4-bit width; an offset back from the output's end and a
/// length, each that many bits, low bit first).
pub fn unpack(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let size = bytes.get(4..8).ok_or(Error::Truncated {
        offset: 4.min(bytes.len()),
        needed: 4,
    })?;
    let size = i32::from_le_bytes([size[0], size[1], size[2], size[3]]);
    let size = usize::try_from(size).map_err(|_| Error::Invalid {
        offset: 4,
        what: format!("an unpacked size of {size} bytes"),
    })?;
    if size > MAX_IMAGE {
        return Err(damaged(format!(
            "it says it unpacks to {size} bytes, more than a Guitar Pro 6 file holds"
        )));
    }
    let ended = || damaged("its compressed data ends before the size it declares");
    let mut bits = Bits {
        bytes: &bytes[8..],
        at: 0,
    };
    // Each bit of input yields at most a few thousand bytes, so reserving by
    // the input keeps a lying header from reserving the cap.
    let mut out = Vec::with_capacity(size.min(bytes.len().saturating_mul(8)));
    while out.len() < size {
        if bits.bit().ok_or_else(ended)? == 1 {
            let width = bits.high_first(4).ok_or_else(ended)?;
            let offset = bits.low_first(width).ok_or_else(ended)? as usize;
            let length = bits.low_first(width).ok_or_else(ended)? as usize;
            let count = offset.min(length).min(size - out.len());
            if count == 0 {
                continue;
            }
            let start = out.len().checked_sub(offset).ok_or_else(|| {
                damaged(format!(
                    "a copy reaches {offset} bytes back from byte {}",
                    out.len()
                ))
            })?;
            // `count <= offset`, so the bytes copied are all there already.
            out.extend_from_within(start..start + count);
        } else {
            for _ in 0..bits.low_first(2).ok_or_else(ended)? {
                out.push(bits.high_first(8).ok_or_else(ended)? as u8);
            }
        }
    }
    out.truncate(size);
    Ok(out)
}

/// An entry of the file system: a directory or a file, and the sectors of
/// its data.
struct Entry {
    kind: i32,
    name: String,
    size: usize,
    blocks: Vec<usize>,
}

/// A BCFS image's sectors, after its 4-byte magic.
struct Sectors<'a> {
    body: &'a [u8],
}

impl<'a> Sectors<'a> {
    fn new(image: &'a [u8]) -> Result<Self, Error> {
        match image.strip_prefix(b"BCFS") {
            Some(body) => Ok(Sectors { body }),
            None => Err(damaged("its file system does not begin with BCFS")),
        }
    }

    fn int(&self, at: usize) -> Option<i32> {
        let b = self.body.get(at..at.checked_add(4)?)?;
        Some(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// The bytes of sector `index`; the image's last may be short.
    fn sector(&self, index: usize) -> Option<&'a [u8]> {
        let start = index.checked_mul(SECTOR)?;
        if start >= self.body.len() {
            return None;
        }
        Some(&self.body[start..(start + SECTOR).min(self.body.len())])
    }

    fn entry(&self, index: usize) -> Result<Entry, Error> {
        let sector = self
            .sector(index)
            .filter(|s| s.len() >= BLOCKS)
            .ok_or_else(|| damaged(format!("its entry at sector {index} is past its end")))?;
        let at = index * SECTOR;
        let kind = self.int(at).unwrap_or_default();
        let name = &sector[NAME..NAME + NAME_LEN];
        let name = &name[..name.iter().position(|&b| b == 0).unwrap_or(NAME_LEN)];
        let name = String::from_utf8_lossy(name).into_owned();
        let size = self.int(at + SIZE).unwrap_or_default();
        let size = usize::try_from(size)
            .map_err(|_| damaged(format!("its entry {name:?} has a size of {size} bytes")))?;
        let mut blocks = Vec::new();
        for slot in (BLOCKS..sector.len().saturating_sub(3)).step_by(4) {
            let block = self.int(at + slot).unwrap_or_default();
            if block == 0 {
                break;
            }
            match usize::try_from(block)
                .ok()
                .filter(|&b| self.sector(b).is_some())
            {
                Some(block) => blocks.push(block),
                None => {
                    return Err(damaged(format!(
                        "its entry {name:?} names sector {block}, which it does not have"
                    )));
                }
            }
        }
        Ok(Entry {
            kind,
            name,
            size,
            blocks,
        })
    }

    /// An entry's data: its sectors in order, cut to its size.
    fn content(&self, entry: &Entry) -> Result<Vec<u8>, Error> {
        let mut out = Vec::with_capacity(entry.size.min(self.body.len()));
        for &block in &entry.blocks {
            if out.len() >= entry.size {
                break;
            }
            let sector = self.sector(block).unwrap_or_default();
            out.extend_from_slice(&sector[..sector.len().min(entry.size - out.len())]);
        }
        if out.len() < entry.size {
            return Err(damaged(format!(
                "its file {:?} says it has {} bytes but its sectors hold {}",
                entry.name,
                entry.size,
                out.len()
            )));
        }
        Ok(out)
    }
}

/// The files of a BCFS image, as paths from the root (`score.gpif`,
/// `Content/x`), each with the sector of its entry. Walks the directories
/// from the root at sector 1 and visits each entry once.
fn walk(sectors: &Sectors) -> Result<Vec<(String, Entry)>, Error> {
    let mut files = Vec::new();
    let mut seen = HashSet::new();
    let mut stack = vec![(1usize, String::new(), 0usize)];
    while let Some((index, prefix, depth)) = stack.pop() {
        if !seen.insert(index) {
            continue;
        }
        let entry = sectors.entry(index)?;
        if index == 1 && entry.kind != KIND_DIRECTORY {
            return Err(damaged(format!(
                "its root directory at sector 1 is an entry of kind {}",
                entry.kind
            )));
        }
        match entry.kind {
            KIND_FILE => files.push((format!("{prefix}{}", entry.name), entry)),
            KIND_DIRECTORY => {
                if depth >= MAX_DEPTH {
                    return Err(damaged("its directories nest too deep"));
                }
                let prefix = if index == 1 {
                    String::new()
                } else {
                    format!("{prefix}{}/", entry.name)
                };
                let data = sectors.content(&entry)?;
                let children: Vec<usize> = data
                    .chunks_exact(4)
                    .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .take_while(|&child| child != 0)
                    .map(|child| {
                        usize::try_from(child).map_err(|_| {
                            damaged(format!(
                                "its directory {:?} lists sector {child}",
                                entry.name
                            ))
                        })
                    })
                    .collect::<Result<_, _>>()?;
                // In reverse, so the stack visits them in the directory's order.
                for child in children.into_iter().rev() {
                    stack.push((child, prefix.clone(), depth + 1));
                }
            }
            kind => {
                return Err(damaged(format!(
                    "its entry {:?} at sector {index} is of kind {kind}",
                    entry.name
                )));
            }
        }
    }
    Ok(files)
}

/// The paths of a BCFS image's files, in the directories' order.
#[cfg(test)]
pub fn paths(image: &[u8]) -> Result<Vec<String>, Error> {
    Ok(walk(&Sectors::new(image)?)?
        .into_iter()
        .map(|(path, _)| path)
        .collect())
}

/// The content of the file called `name` in a BCFS image: at the root, or
/// failing that the first in a directory.
pub fn file(image: &[u8], name: &str) -> Result<Vec<u8>, Error> {
    let sectors = Sectors::new(image)?;
    let files = walk(&sectors)?;
    let found = files
        .iter()
        .find(|(path, _)| path == name)
        .or_else(|| {
            files
                .iter()
                .find(|(path, _)| path.rsplit('/').next() == Some(name))
        })
        .ok_or_else(|| damaged(format!("it has no {name}")))?;
    sectors.content(&found.1)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Writes bits as `unpack` reads them, to build BCFZ by hand.
    #[derive(Default)]
    pub(crate) struct BitWriter {
        bytes: Vec<u8>,
        len: usize,
    }

    impl BitWriter {
        fn bit(&mut self, bit: u32) {
            if self.len.is_multiple_of(8) {
                self.bytes.push(0);
            }
            if bit != 0 {
                *self.bytes.last_mut().unwrap() |= 0x80 >> (self.len % 8);
            }
            self.len += 1;
        }

        fn high_first(&mut self, value: u32, n: u32) {
            for i in (0..n).rev() {
                self.bit((value >> i) & 1);
            }
        }

        fn low_first(&mut self, value: u32, n: u32) {
            for i in 0..n {
                self.bit((value >> i) & 1);
            }
        }

        pub(crate) fn literal(&mut self, bytes: &[u8]) {
            for chunk in bytes.chunks(3) {
                self.bit(0);
                self.low_first(chunk.len() as u32, 2);
                for &b in chunk {
                    self.high_first(b.into(), 8);
                }
            }
        }

        fn copy(&mut self, width: u32, offset: u32, length: u32) {
            self.bit(1);
            self.high_first(width, 4);
            self.low_first(offset, width);
            self.low_first(length, width);
        }

        pub(crate) fn finish(self, size: usize) -> Vec<u8> {
            let mut out = b"BCFZ".to_vec();
            out.extend_from_slice(&(size as i32).to_le_bytes());
            out.extend(self.bytes);
            out
        }
    }

    /// BCFZ of `bytes` in literals only.
    pub(crate) fn compress(bytes: &[u8]) -> Vec<u8> {
        let mut w = BitWriter::default();
        w.literal(bytes);
        w.finish(bytes.len())
    }

    #[test]
    fn unpacks_literals_and_copies() {
        let mut w = BitWriter::default();
        w.literal(b"abc");
        w.copy(3, 3, 6); // min(3, 6): "abc" again
        w.copy(4, 6, 4); // "abca"
        w.copy(0, 0, 0); // copies nothing
        w.literal(b"Z");
        assert_eq!(unpack(&w.finish(11)).unwrap(), b"abcabcabcaZ");
    }

    #[test]
    fn unpacking_stops_at_the_declared_size() {
        let mut w = BitWriter::default();
        w.literal(b"abcdef");
        w.copy(4, 6, 6);
        assert_eq!(unpack(&w.finish(8)).unwrap(), b"abcdefab");
    }

    #[test]
    fn a_damaged_stream_is_an_error() {
        // A copy from before the output began.
        let mut w = BitWriter::default();
        w.literal(b"ab");
        w.copy(3, 5, 2);
        assert!(matches!(unpack(&w.finish(4)), Err(Error::Container(_))));
        // A stream that ends before its size.
        assert!(matches!(
            unpack(&compress(b"abc")[..9]),
            Err(Error::Container(_))
        ));
        let mut short = compress(b"abc");
        short[4] = 9;
        assert!(matches!(unpack(&short), Err(Error::Container(_))));
        // A size that cannot be.
        assert!(matches!(
            unpack(b"BCFZ\xff\xff\xff\xff"),
            Err(Error::Invalid { offset: 4, .. })
        ));
        assert!(matches!(
            unpack(b"BCFZ\xff\xff\xff\x7f"),
            Err(Error::Container(_))
        ));
        assert!(matches!(unpack(b"BCFZ\x01"), Err(Error::Truncated { .. })));
        // Nothing to read is no error when nothing is declared.
        assert_eq!(unpack(b"BCFZ\0\0\0\0").unwrap(), b"");
    }

    /// A BCFS image of `sectors` zeroed sectors, to fill by hand.
    pub(crate) struct Image(Vec<u8>);

    impl Image {
        pub(crate) fn new(sectors: usize) -> Self {
            Image(vec![0; SECTOR * sectors])
        }

        pub(crate) fn int(&mut self, at: usize, value: i32) {
            self.0[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }

        pub(crate) fn entry(
            &mut self,
            sector: usize,
            kind: i32,
            name: &str,
            size: usize,
            blocks: &[i32],
        ) {
            let at = sector * SECTOR;
            self.int(at, kind);
            self.0[at + NAME..at + NAME + name.len()].copy_from_slice(name.as_bytes());
            self.int(at + 0x88, 1);
            self.int(at + SIZE, size as i32);
            for (i, &block) in blocks.iter().enumerate() {
                self.int(at + BLOCKS + 4 * i, block);
            }
        }

        pub(crate) fn data(&mut self, sector: usize, bytes: &[u8]) {
            let at = sector * SECTOR;
            self.0[at..at + bytes.len()].copy_from_slice(bytes);
        }

        pub(crate) fn bytes(&self) -> Vec<u8> {
            let mut out = b"BCFS".to_vec();
            out.extend_from_slice(&self.0);
            out
        }
    }

    /// The sample's shape (README §3.2): the root at sector 1 whose data
    /// sector begins with `2`, a file entry at 2, and `score.gpif` over two
    /// sectors.
    pub(crate) fn sample_image(score: &[u8]) -> Image {
        let mut image = Image::new(8);
        image.entry(1, 1, "/", 8, &[3]);
        image.entry(2, 2, "misc.xml", 5, &[4]);
        let mut dir = Vec::new();
        dir.extend_from_slice(&2i32.to_le_bytes());
        dir.extend_from_slice(&5i32.to_le_bytes());
        image.data(3, &dir);
        image.data(4, b"<x/>\n");
        let blocks: Vec<i32> = (6..8).collect();
        image.entry(5, 2, "score.gpif", score.len(), &blocks);
        image.data(6, &score[..score.len().min(SECTOR)]);
        if score.len() > SECTOR {
            image.data(7, &score[SECTOR..]);
        }
        image
    }

    #[test]
    fn walks_the_directory_from_the_root() {
        let score: Vec<u8> = (0..SECTOR + 10).map(|i| (i % 251) as u8).collect();
        let image = sample_image(&score).bytes();
        // Sector 3, the root's data, begins with 2 as a file entry would; a
        // walk does not take it for one.
        assert_eq!(paths(&image).unwrap(), ["misc.xml", "score.gpif"]);
        assert_eq!(file(&image, "score.gpif").unwrap(), score);
        assert_eq!(file(&image, "misc.xml").unwrap(), b"<x/>\n");
        assert!(matches!(
            file(&image, "LayoutConfiguration"),
            Err(Error::Container(_))
        ));
        // The same image compressed.
        let packed = compress(&image);
        assert_eq!(super::image(&packed).unwrap().as_ref(), &image[..]);
    }

    #[test]
    fn finds_files_in_subdirectories() {
        let mut image = Image::new(6);
        image.entry(1, 1, "/", 4, &[2]);
        image.data(2, &3i32.to_le_bytes());
        image.entry(3, 1, "Content", 4, &[4]);
        image.data(4, &5i32.to_le_bytes());
        image.entry(5, 2, "score.gpif", 0, &[]);
        let image = image.bytes();
        assert_eq!(paths(&image).unwrap(), ["Content/score.gpif"]);
        assert_eq!(file(&image, "score.gpif").unwrap(), b"");
    }

    #[test]
    fn a_damaged_file_system_is_an_error() {
        // A directory that lists itself is walked once.
        let mut image = Image::new(3);
        image.entry(1, 1, "/", 8, &[2]);
        image.data(2, &[1, 0, 0, 0, 1, 0, 0, 0]);
        assert_eq!(paths(&image.bytes()).unwrap(), Vec::<String>::new());
        // A file larger than its sectors.
        let mut image = sample_image(b"abc");
        image.entry(5, 2, "score.gpif", 3 * SECTOR, &[6, 7]);
        assert!(matches!(
            file(&image.bytes(), "score.gpif"),
            Err(Error::Container(_))
        ));
        // A sector past the end.
        let mut image = sample_image(b"abc");
        image.entry(5, 2, "score.gpif", 3, &[99]);
        assert!(matches!(paths(&image.bytes()), Err(Error::Container(_))));
        // A negative size, an unknown kind, a root that is not a directory.
        let mut image = sample_image(b"abc");
        image.int(5 * SECTOR + SIZE, -1);
        assert!(matches!(paths(&image.bytes()), Err(Error::Container(_))));
        let mut image = sample_image(b"abc");
        image.int(5 * SECTOR, 7);
        assert!(matches!(paths(&image.bytes()), Err(Error::Container(_))));
        let mut image = sample_image(b"abc");
        image.int(SECTOR, 2);
        assert!(matches!(paths(&image.bytes()), Err(Error::Container(_))));
        // Too short to hold the root, or not BCFS at all.
        assert!(matches!(paths(b"BCFS"), Err(Error::Container(_))));
        assert!(matches!(paths(b"PK\x03\x04"), Err(Error::Container(_))));
        assert!(matches!(
            super::image(b"BCFZ\x04\0\0\0\x0f"),
            Err(Error::Container(_))
        ));
        assert!(matches!(
            super::image(b"BCFZ\0\0\0\0"),
            Err(Error::Container(_))
        ));
    }
}
