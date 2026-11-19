//! The WAV aligner of Export for GarageBand (D162, README §6.3 and §8 stage 2
//! of `docs/research/garageband`): a WAV padded with silence or cut at its
//! head, so its sample 0 is the MIDI's tick 0 and every file the export
//! writes can be dropped at bar 1 together. It is done at the file's own rate
//! and depth (GarageBand takes 48 kHz, §4.4), frame by frame, so the samples
//! themselves are the file's, unchanged.
//!
//! It reads PCM of 8, 16, 24 or 32 bits and float of 32 or 64, plain or as
//! `WAVE_FORMAT_EXTENSIBLE` (ffmpeg writes 24-bit so). The file it writes is
//! `RIFF`/`WAVE` with the `fmt ` chunk as it was, a `fact` chunk for float,
//! and the `data` chunk: other chunks (`LIST`, `bext`, …) are left out.
//! `make_import_bundle.py`'s `align` is the reference.
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

const WAVE_FORMAT_PCM: u16 = 0x0001;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

/// How a WAV's samples are stored, from its `fmt ` chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WavFormat {
    /// True for IEEE float, false for integer PCM.
    pub float: bool,
    pub channels: u16,
    /// Frames a second.
    pub rate: u32,
    /// Bits a sample: 8, 16, 24 or 32 for PCM, 32 or 64 for float.
    pub bits: u16,
}

impl WavFormat {
    /// Bytes a frame: a sample of each channel.
    pub fn frame_bytes(&self) -> usize {
        self.channels as usize * (self.bits as usize / 8)
    }

    /// The byte a silent sample is made of: 8-bit PCM is unsigned, so its 0
    /// is 0x80; every other depth's 0, float's too, is all zero bits.
    fn silence(&self) -> u8 {
        if !self.float && self.bits == 8 {
            0x80
        } else {
            0
        }
    }
}

/// What `align` wrote: the file's format and its frames before and after.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Aligned {
    pub format: WavFormat,
    /// Frames of the file read.
    pub frames_in: u64,
    /// Frames of the file written: `frames_in` plus the silence, or less the
    /// head cut, and none when the cut is the whole file.
    pub frames_out: u64,
}

/// The frames `shift` seconds make at `rate`, rounded as the reference does:
/// above 0 the silence put before the file, below 0 the frames cut from its
/// head.
pub fn shift_frames(shift: f64, rate: u32) -> i64 {
    (shift * f64::from(rate)).round() as i64
}

/// The WAV at `from` aligned by `shift` seconds and written to `to`: padded
/// with that much silence when `shift` is above 0, cut by that much at its
/// head when below. `to` must not be `from`.
pub fn align_file(from: &Path, to: &Path, shift: f64) -> Result<Aligned, String> {
    let name = from.file_name().map_or_else(
        || from.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let input = File::open(from).map_err(|e| format!("{name} could not be read: {e}."))?;
    // Creating `to` would empty `from` before it is read.
    if std::fs::canonicalize(to)
        .is_ok_and(|to| std::fs::canonicalize(from).is_ok_and(|from| from == to))
    {
        return Err(format!(
            "{name} could not be aligned: it would be written over itself."
        ));
    }
    let output = File::create(to).map_err(|e| {
        format!(
            "{} could not be written: {e}.",
            to.file_name().unwrap_or_default().to_string_lossy()
        )
    })?;
    let mut writer = BufWriter::new(output);
    let aligned = align(&mut BufReader::new(input), &mut writer, shift)
        .and_then(|aligned| {
            writer
                .flush()
                .map(|()| aligned)
                .map_err(|e| format!("it could not be written ({e})"))
        })
        .map_err(|why| format!("{name} could not be aligned: {why}."));
    if aligned.is_err() {
        // No half-written WAV left behind.
        drop(writer);
        let _ = std::fs::remove_file(to);
    }
    aligned
}

/// The WAV read from `input` aligned by `shift` seconds and written to
/// `output`, as `align_file` does. The error says why, without the file's
/// name or a full stop, for the caller to put in a sentence.
pub fn align<R: Read + Seek, W: Write>(
    input: &mut R,
    output: &mut W,
    shift: f64,
) -> Result<Aligned, String> {
    if !shift.is_finite() {
        return Err("its shift is not a number".into());
    }
    let (fmt, format, data_len) = read_header(input)?;
    let frame = format.frame_bytes() as u64;
    let frames_in = data_len / frame;
    let n = shift_frames(shift, format.rate);
    let (pad, cut) = if n >= 0 {
        (n.unsigned_abs(), 0)
    } else {
        (0, n.unsigned_abs().min(frames_in))
    };
    let fact = format.float;
    let too_long = || "it would be longer than a WAV file can be (4 GB)".to_string();
    // RIFF's size: `WAVE`, the chunks and their headers, data's pad byte;
    // checked, as a shift of years would overflow before it is refused.
    let frames_out = (frames_in - cut).checked_add(pad).ok_or_else(too_long)?;
    let out_len = frames_out.checked_mul(frame).ok_or_else(too_long)?;
    let riff_len = (4 + 8 + padded(fmt.len() as u64) + if fact { 12 } else { 0 } + 8)
        .checked_add(padded(out_len))
        .ok_or_else(too_long)?;
    let riff_len = u32::try_from(riff_len).map_err(|_| too_long())?;
    let frames_out_u32 = u32::try_from(frames_out).unwrap_or(u32::MAX);

    let write_err = |e: io::Error| format!("it could not be written ({e})");
    let mut head = Vec::with_capacity(64);
    head.extend_from_slice(b"RIFF");
    head.extend_from_slice(&riff_len.to_le_bytes());
    head.extend_from_slice(b"WAVE");
    head.extend_from_slice(b"fmt ");
    head.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
    head.extend_from_slice(&fmt);
    if fmt.len() % 2 == 1 {
        head.push(0);
    }
    if fact {
        head.extend_from_slice(b"fact");
        head.extend_from_slice(&4u32.to_le_bytes());
        head.extend_from_slice(&frames_out_u32.to_le_bytes());
    }
    head.extend_from_slice(b"data");
    head.extend_from_slice(&(out_len as u32).to_le_bytes());
    output.write_all(&head).map_err(write_err)?;

    // The silence, a block at a time.
    let block = vec![format.silence(); 64 * 1024 / frame as usize * frame as usize];
    let mut left = pad * frame;
    while left > 0 {
        let take = left.min(block.len() as u64) as usize;
        output.write_all(&block[..take]).map_err(write_err)?;
        left -= take as u64;
    }
    // The frames kept, past the cut.
    input
        .seek(SeekFrom::Current((cut * frame) as i64))
        .map_err(|e| format!("it could not be read ({e})"))?;
    let keep = (frames_in - cut) * frame;
    let copied = io::copy(&mut input.take(keep), output).map_err(write_err)?;
    if copied != keep {
        return Err("it ends before its data".into());
    }
    if out_len % 2 == 1 {
        output.write_all(&[0]).map_err(write_err)?;
    }
    Ok(Aligned {
        format,
        frames_in,
        frames_out,
    })
}

/// A chunk's length with its pad byte: chunks start at even offsets.
fn padded(len: u64) -> u64 {
    len + len % 2
}

/// The `fmt ` chunk's bytes, the format they describe, and the length of the
/// `data` chunk, whole frames only, with `input` left at its first byte. A
/// `data` length past the end of the file (0xFFFFFFFF from a writer that
/// could not seek back) is taken to the end.
fn read_header<R: Read + Seek>(input: &mut R) -> Result<(Vec<u8>, WavFormat, u64), String> {
    let not_wav = || "it is not a WAV file".to_string();
    let short = || "it is cut short".to_string();
    let end = input
        .seek(SeekFrom::End(0))
        .and_then(|end| input.seek(SeekFrom::Start(0)).map(|_| end))
        .map_err(|e| format!("it could not be read ({e})"))?;
    let mut riff = [0u8; 12];
    input.read_exact(&mut riff).map_err(|_| not_wav())?;
    if &riff[0..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
        return Err(not_wav());
    }
    let mut at = 12u64;
    let mut fmt: Option<(Vec<u8>, WavFormat)> = None;
    loop {
        let mut header = [0u8; 8];
        if input.read_exact(&mut header).is_err() {
            return Err(if fmt.is_some() {
                "it has no data".into()
            } else {
                "it has no format".into()
            });
        }
        at += 8;
        let len = u64::from(u32::from_le_bytes(header[4..8].try_into().unwrap()));
        match &header[0..4] {
            b"fmt " => {
                if len < 16 || at + len > end {
                    return Err(short());
                }
                let mut body = vec![0u8; len as usize];
                input.read_exact(&mut body).map_err(|_| short())?;
                fmt = Some((body.clone(), parse_format(&body)?));
            }
            b"data" => {
                let Some((body, format)) = fmt else {
                    return Err("its data comes before its format".into());
                };
                let len = len.min(end.saturating_sub(at));
                let frame = format.frame_bytes() as u64;
                return Ok((body, format, len / frame * frame));
            }
            _ => {
                if at + len > end {
                    return Err(short());
                }
                input
                    .seek(SeekFrom::Current(len as i64))
                    .map_err(|_| short())?;
            }
        }
        at += len;
        if len % 2 == 1 {
            input.seek(SeekFrom::Current(1)).map_err(|_| short())?;
            at += 1;
        }
    }
}

/// The format a `fmt ` chunk describes, or why it is not one the aligner
/// takes: compressed WAV (ADPCM, µ-law, …) has no frames of a fixed size.
fn parse_format(body: &[u8]) -> Result<WavFormat, String> {
    let u16_at = |i: usize| u16::from_le_bytes([body[i], body[i + 1]]);
    let mut tag = u16_at(0);
    let channels = u16_at(2);
    let rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
    let block_align = u16_at(12);
    let bits = u16_at(14);
    if tag == WAVE_FORMAT_EXTENSIBLE {
        // cbSize, valid bits, channel mask, then the subformat's GUID,
        // whose first two bytes are the format's tag.
        if body.len() < 40 {
            return Err("its format is cut short".into());
        }
        tag = u16_at(24);
    }
    let float = match (tag, bits) {
        (WAVE_FORMAT_PCM, 8 | 16 | 24 | 32) => false,
        (WAVE_FORMAT_IEEE_FLOAT, 32 | 64) => true,
        (WAVE_FORMAT_PCM | WAVE_FORMAT_IEEE_FLOAT, _) => {
            return Err(format!("it has {bits}-bit samples"));
        }
        _ => return Err(format!("it is compressed (format {tag:#06x})")),
    };
    let format = WavFormat {
        float,
        channels,
        rate,
        bits,
    };
    if channels == 0 || rate == 0 {
        return Err("it has no channels or no rate".into());
    }
    if block_align as usize != format.frame_bytes() {
        return Err("its frames are not the size its format says".into());
    }
    Ok(format)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A sample as the file stores it, from a value in -1..1.
    fn encode(format: &WavFormat, value: f64) -> Vec<u8> {
        match (format.float, format.bits) {
            (true, 32) => (value as f32).to_le_bytes().to_vec(),
            (true, 64) => value.to_le_bytes().to_vec(),
            (false, 8) => vec![(128.0 + value * 127.0).round() as u8],
            (false, 16) => ((value * 32767.0).round() as i16).to_le_bytes().to_vec(),
            (false, 24) => ((value * 8_388_607.0).round() as i32).to_le_bytes()[..3].to_vec(),
            (false, 32) => ((value * 2_147_483_647.0).round() as i32)
                .to_le_bytes()
                .to_vec(),
            _ => unreachable!(),
        }
    }

    /// A sample read back to a value in -1..1.
    fn decode(format: &WavFormat, bytes: &[u8]) -> f64 {
        match (format.float, format.bits) {
            (true, 32) => f64::from(f32::from_le_bytes(bytes.try_into().unwrap())),
            (true, 64) => f64::from_le_bytes(bytes.try_into().unwrap()),
            (false, 8) => (f64::from(bytes[0]) - 128.0) / 127.0,
            (false, 16) => f64::from(i16::from_le_bytes(bytes.try_into().unwrap())) / 32767.0,
            (false, 24) => {
                f64::from(i32::from_le_bytes([0, bytes[0], bytes[1], bytes[2]]) >> 8) / 8_388_607.0
            }
            (false, 32) => {
                f64::from(i32::from_le_bytes(bytes.try_into().unwrap())) / 2_147_483_647.0
            }
            _ => unreachable!(),
        }
    }

    fn chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut chunk = tag.to_vec();
        chunk.extend_from_slice(&(data.len() as u32).to_le_bytes());
        chunk.extend_from_slice(data);
        if data.len() % 2 == 1 {
            chunk.push(0);
        }
        chunk
    }

    fn fmt_body(format: &WavFormat, extensible: bool) -> Vec<u8> {
        let tag = if format.float {
            WAVE_FORMAT_IEEE_FLOAT
        } else {
            WAVE_FORMAT_PCM
        };
        let frame = format.frame_bytes() as u32;
        let mut body = Vec::new();
        body.extend_from_slice(
            &(if extensible {
                WAVE_FORMAT_EXTENSIBLE
            } else {
                tag
            })
            .to_le_bytes(),
        );
        body.extend_from_slice(&format.channels.to_le_bytes());
        body.extend_from_slice(&format.rate.to_le_bytes());
        body.extend_from_slice(&(format.rate * frame).to_le_bytes());
        body.extend_from_slice(&(frame as u16).to_le_bytes());
        body.extend_from_slice(&format.bits.to_le_bytes());
        if extensible {
            body.extend_from_slice(&22u16.to_le_bytes());
            body.extend_from_slice(&format.bits.to_le_bytes());
            body.extend_from_slice(&3u32.to_le_bytes());
            body.extend_from_slice(&tag.to_le_bytes());
            body.extend_from_slice(&[
                0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
            ]);
        }
        body
    }

    /// Frame `i`, channel `c`: a value no other sample of the file has.
    fn value(i: usize, c: usize) -> f64 {
        let v = (i * 2 + c + 1) as f64 / 64.0;
        if c.is_multiple_of(2) { v } else { -v }
    }

    /// A WAV of `frames` frames of `value`, with a `LIST` chunk of odd length
    /// between its format and its data, as writers put tags there.
    fn wav(format: &WavFormat, frames: usize, extensible: bool) -> Vec<u8> {
        let mut data = Vec::new();
        for i in 0..frames {
            for c in 0..format.channels as usize {
                data.extend(encode(format, value(i, c)));
            }
        }
        let mut body = b"WAVE".to_vec();
        body.extend(chunk(b"fmt ", &fmt_body(format, extensible)));
        body.extend(chunk(b"LIST", b"INFOISFT\x03\0\0\0Lf\0"));
        body.extend(chunk(b"data", &data));
        chunk(b"RIFF", &body)
    }

    /// The file's chunks in order: their tags and bodies, checking the RIFF
    /// size and that each odd chunk has its pad byte.
    fn chunks(bytes: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
        assert_eq!(&bytes[0..4], b"RIFF");
        let riff_len = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        assert_eq!(riff_len + 8, bytes.len(), "RIFF's size");
        assert_eq!(&bytes[8..12], b"WAVE");
        let mut at = 12;
        let mut out = Vec::new();
        while at < bytes.len() {
            let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
            out.push((
                bytes[at..at + 4].try_into().unwrap(),
                bytes[at + 8..at + 8 + len].to_vec(),
            ));
            at += 8 + len + len % 2;
        }
        assert_eq!(at, bytes.len());
        out
    }

    /// The written file's frames, decoded, with its chunks checked: `fmt `
    /// as it was, `fact` with the frames for float only, then `data`.
    fn frames_of(bytes: &[u8], format: &WavFormat, fmt: &[u8]) -> Vec<Vec<f64>> {
        let chunks = chunks(bytes);
        let tags: Vec<&[u8; 4]> = chunks.iter().map(|(tag, _)| tag).collect();
        let data = &chunks.last().unwrap().1;
        let frame = format.frame_bytes();
        assert_eq!(chunks[0].1, fmt, "fmt as it was");
        if format.float {
            assert_eq!(tags, [b"fmt ", b"fact", b"data"]);
            assert_eq!(
                u32::from_le_bytes(chunks[1].1[..].try_into().unwrap()) as usize,
                data.len() / frame
            );
        } else {
            assert_eq!(tags, [b"fmt ", b"data"]);
        }
        assert_eq!(data.len() % frame, 0);
        let sample = format.bits as usize / 8;
        data.chunks(frame)
            .map(|f| f.chunks(sample).map(|s| decode(format, s)).collect())
            .collect()
    }

    fn expected_frame(format: &WavFormat, i: usize) -> Vec<f64> {
        (0..format.channels as usize)
            .map(|c| decode(format, &encode(format, value(i, c))))
            .collect()
    }

    const FORMATS: [WavFormat; 4] = [
        WavFormat {
            float: false,
            channels: 2,
            rate: 44_100,
            bits: 16,
        },
        WavFormat {
            float: false,
            channels: 2,
            rate: 48_000,
            bits: 24,
        },
        WavFormat {
            float: true,
            channels: 2,
            rate: 44_100,
            bits: 32,
        },
        WavFormat {
            float: false,
            channels: 1,
            rate: 22_050,
            bits: 24,
        },
    ];

    #[test]
    fn pads_silence_at_each_depth() {
        for format in FORMATS {
            for extensible in [false, true] {
                let input = wav(&format, 20, extensible);
                let mut out = Vec::new();
                // 5 frames of silence, and a fraction of one rounded away.
                let shift = (5.0 + 0.3) / f64::from(format.rate);
                let aligned = align(&mut Cursor::new(&input), &mut out, shift).unwrap();
                assert_eq!(
                    aligned,
                    Aligned {
                        format,
                        frames_in: 20,
                        frames_out: 25
                    }
                );
                let frames = frames_of(&out, &format, &fmt_body(&format, extensible));
                assert_eq!(frames.len(), 25, "{format:?}");
                for frame in &frames[..5] {
                    assert!(frame.iter().all(|&s| s == 0.0), "{format:?}: {frame:?}");
                }
                for (i, frame) in frames[5..].iter().enumerate() {
                    assert_eq!(*frame, expected_frame(&format, i), "{format:?} frame {i}");
                }
            }
        }
    }

    #[test]
    fn cuts_the_head_at_each_depth() {
        for format in FORMATS {
            for extensible in [false, true] {
                let input = wav(&format, 20, extensible);
                let mut out = Vec::new();
                let shift = -7.0 / f64::from(format.rate);
                let aligned = align(&mut Cursor::new(&input), &mut out, shift).unwrap();
                assert_eq!((aligned.frames_in, aligned.frames_out), (20, 13));
                let frames = frames_of(&out, &format, &fmt_body(&format, extensible));
                let expected: Vec<Vec<f64>> = (7..20).map(|i| expected_frame(&format, i)).collect();
                assert_eq!(frames, expected, "{format:?}");
            }
        }
    }

    #[test]
    fn a_zero_shift_keeps_every_frame() {
        let format = FORMATS[0];
        let input = wav(&format, 9, false);
        let mut out = Vec::new();
        align(&mut Cursor::new(&input), &mut out, 0.0).unwrap();
        let frames = frames_of(&out, &format, &fmt_body(&format, false));
        let expected: Vec<Vec<f64>> = (0..9).map(|i| expected_frame(&format, i)).collect();
        assert_eq!(frames, expected);
    }

    #[test]
    fn a_cut_past_the_end_leaves_no_frames() {
        let format = FORMATS[2];
        let input = wav(&format, 10, false);
        let mut out = Vec::new();
        let aligned = align(&mut Cursor::new(&input), &mut out, -1.0).unwrap();
        assert_eq!((aligned.frames_in, aligned.frames_out), (10, 0));
        assert!(frames_of(&out, &format, &fmt_body(&format, false)).is_empty());
    }

    #[test]
    fn eight_bit_silence_is_its_middle() {
        let format = WavFormat {
            float: false,
            channels: 1,
            rate: 8_000,
            bits: 8,
        };
        // 3 frames in, 2 padded: 5 bytes of data, so a pad byte after them.
        let input = wav(&format, 3, false);
        let mut out = Vec::new();
        align(&mut Cursor::new(&input), &mut out, 2.0 / 8_000.0).unwrap();
        let chunks = chunks(&out);
        let data = &chunks.last().unwrap().1;
        assert_eq!(data.len(), 5);
        assert_eq!(&data[..2], [0x80, 0x80]);
        assert_eq!(*out.last().unwrap(), 0, "the pad byte");
    }

    #[test]
    fn a_data_length_past_the_end_is_taken_to_the_end() {
        let format = FORMATS[0];
        let mut input = wav(&format, 6, false);
        // As a writer that streams leaves it, with half a frame at the end.
        let data_at = input.windows(4).position(|w| w == b"data").unwrap();
        input[data_at + 4..data_at + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        input.extend_from_slice(&[1, 2]);
        let mut out = Vec::new();
        let aligned = align(&mut Cursor::new(&input), &mut out, -2.0 / 44_100.0).unwrap();
        assert_eq!((aligned.frames_in, aligned.frames_out), (6, 4));
        let frames = frames_of(&out, &format, &fmt_body(&format, false));
        let expected: Vec<Vec<f64>> = (2..6).map(|i| expected_frame(&format, i)).collect();
        assert_eq!(frames, expected);
    }

    #[test]
    fn refuses_what_it_cannot_align() {
        let refused =
            |bytes: &[u8]| align(&mut Cursor::new(bytes), &mut Vec::new(), 0.5).unwrap_err();
        assert_eq!(refused(b"ID3\x04 an mp3"), "it is not a WAV file");
        assert_eq!(
            refused(b"fLaC\0\0\0\x22 a flac file"),
            "it is not a WAV file"
        );

        let mut adpcm = fmt_body(&FORMATS[0], false);
        adpcm[0..2].copy_from_slice(&2u16.to_le_bytes());
        let mut body = b"WAVE".to_vec();
        body.extend(chunk(b"fmt ", &adpcm));
        body.extend(chunk(b"data", &[0; 8]));
        assert_eq!(
            refused(&chunk(b"RIFF", &body)),
            "it is compressed (format 0x0002)"
        );

        let mut body = b"WAVE".to_vec();
        body.extend(chunk(b"data", &[0; 8]));
        assert_eq!(
            refused(&chunk(b"RIFF", &body)),
            "its data comes before its format"
        );

        let mut body = b"WAVE".to_vec();
        body.extend(chunk(b"fmt ", &fmt_body(&FORMATS[0], false)));
        assert_eq!(refused(&chunk(b"RIFF", &body)), "it has no data");

        let input = wav(&FORMATS[0], 4, false);
        let shift = align(&mut Cursor::new(&input), &mut Vec::new(), f64::NAN).unwrap_err();
        assert_eq!(shift, "its shift is not a number");
        for years in [1e5, 1e12, f64::MAX] {
            let long = align(&mut Cursor::new(&input), &mut Vec::new(), years).unwrap_err();
            assert_eq!(long, "it would be longer than a WAV file can be (4 GB)");
        }
    }

    #[test]
    fn shifts_round_to_the_nearest_frame() {
        assert_eq!(shift_frames(1.5, 48_000), 72_000);
        assert_eq!(shift_frames(-0.25, 44_100), -11_025);
        assert_eq!(shift_frames(0.4 / 44_100.0, 44_100), 0);
        assert_eq!(shift_frames(-0.6 / 44_100.0, 44_100), -1);
    }

    #[test]
    fn aligns_a_file_on_disk() {
        let scratch = tempfile::tempdir().unwrap();
        let format = FORMATS[1];
        let from = scratch.path().join("song - vocals.wav");
        let to = scratch.path().join("Vocals.wav");
        std::fs::write(&from, wav(&format, 12, true)).unwrap();
        let aligned = align_file(&from, &to, 3.0 / 48_000.0).unwrap();
        assert_eq!(aligned.frames_out, 15);
        let frames = frames_of(
            &std::fs::read(&to).unwrap(),
            &format,
            &fmt_body(&format, true),
        );
        assert_eq!(
            frames[3..],
            (0..12)
                .map(|i| expected_frame(&format, i))
                .collect::<Vec<_>>()
        );

        let before = std::fs::read(&from).unwrap();
        assert_eq!(
            align_file(&from, &from, 0.0).unwrap_err(),
            "song - vocals.wav could not be aligned: it would be written over itself."
        );
        assert_eq!(std::fs::read(&from).unwrap(), before);

        let missing = scratch.path().join("gone.wav");
        assert!(
            align_file(&missing, &to, 0.0)
                .unwrap_err()
                .starts_with("gone.wav could not be read: ")
        );
        let flac = scratch.path().join("song - drums.flac");
        std::fs::write(&flac, b"fLaC\0\0\0\x22").unwrap();
        assert_eq!(
            align_file(&flac, &to, 0.0).unwrap_err(),
            "song - drums.flac could not be aligned: it is not a WAV file."
        );
    }
}
