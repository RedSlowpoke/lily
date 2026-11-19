//! Text in GP3–GP5, which declares no encoding (README §2.1): it is the ANSI
//! code page of the machine that saved the file. The reader guesses one
//! encoding for the whole file from all its strings, between UTF-8,
//! Windows-1251 (Cyrillic) and Windows-1252 (western), and decodes with the
//! tables below, written from the code pages' published charts.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Encoding {
    Utf8,
    Windows1251,
    Windows1252,
}

/// The encoding of a file whose strings are `texts`, as stored.
///
/// - All ASCII: Windows-1252, which decodes ASCII as itself.
/// - Every string with a byte ≥ 0x80 valid UTF-8: UTF-8 (a run of 1251 or
///   1252 text is almost never valid UTF-8).
/// - Else Cyrillic or western by the neighbours of the high bytes. Russian
///   in Windows-1251 is words of letters ≥ 0xC0 side by side; western text
///   in Windows-1252 has an accented letter here and there between ASCII
///   letters. So a pair of high letters counts for 1251 and a high byte
///   beside an ASCII letter for 1252.
pub(super) fn guess<'a>(texts: impl IntoIterator<Item = &'a [u8]>) -> Encoding {
    let mut any = false;
    let mut utf8 = true;
    let mut score: i64 = 0;
    for raw in texts {
        if raw.is_ascii() {
            continue;
        }
        any = true;
        utf8 &= std::str::from_utf8(raw).is_ok();
        for pair in raw.windows(2) {
            if let [a, b] = *pair {
                if a >= 0xC0 && b >= 0xC0 {
                    score += 1;
                } else if (a >= 0x80 && b.is_ascii_alphabetic())
                    || (b >= 0x80 && a.is_ascii_alphabetic())
                {
                    score -= 1;
                }
            }
        }
    }
    match (any, utf8) {
        (false, _) => Encoding::Windows1252,
        (true, true) => Encoding::Utf8,
        _ if score > 0 => Encoding::Windows1251,
        _ => Encoding::Windows1252,
    }
}

pub(super) fn decode(raw: &[u8], encoding: Encoding) -> String {
    match encoding {
        Encoding::Utf8 => String::from_utf8_lossy(raw).into_owned(),
        _ => raw.iter().map(|&b| char_of(b, encoding)).collect(),
    }
}

fn char_of(byte: u8, encoding: Encoding) -> char {
    let code = match (encoding, byte) {
        (_, 0x00..=0x7F) => u32::from(byte),
        (Encoding::Windows1251, 0xC0..=0xFF) => 0x0410 + u32::from(byte - 0xC0),
        (Encoding::Windows1251, _) => table(&WINDOWS_1251_80, byte),
        (_, 0x80..=0x9F) => table(&WINDOWS_1252_80, byte),
        // Windows-1252 from 0xA0 is Latin-1, so the byte is the code point.
        _ => u32::from(byte),
    };
    char::from_u32(code).unwrap_or('\u{FFFD}')
}

fn table(chart: &[u16], byte: u8) -> u32 {
    chart
        .get(usize::from(byte.wrapping_sub(0x80)))
        .map_or(0xFFFD, |&c| u32::from(c))
}

/// Windows-1252 from 0x80 to 0x9F. The five bytes the code page leaves
/// undefined (0x81, 0x8D, 0x8F, 0x90, 0x9D) are their C1 controls, as
/// browsers decode them.
const WINDOWS_1252_80: [u16; 32] = [
    0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, //
    0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x008D, 0x017D, 0x008F, //
    0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, //
    0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x009D, 0x017E, 0x0178,
];

/// Windows-1251 from 0x80 to 0xBF; 0xC0–0xFF are А–я (U+0410–U+044F).
/// 0x98, undefined, is its C1 control.
const WINDOWS_1251_80: [u16; 64] = [
    0x0402, 0x0403, 0x201A, 0x0453, 0x201E, 0x2026, 0x2020, 0x2021, //
    0x20AC, 0x2030, 0x0409, 0x2039, 0x040A, 0x040C, 0x040B, 0x040F, //
    0x0452, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, //
    0x0098, 0x2122, 0x0459, 0x203A, 0x045A, 0x045C, 0x045B, 0x045F, //
    0x00A0, 0x040E, 0x045E, 0x0408, 0x00A4, 0x0490, 0x00A6, 0x00A7, //
    0x0401, 0x00A9, 0x0404, 0x00AB, 0x00AC, 0x00AD, 0x00AE, 0x0407, //
    0x00B0, 0x00B1, 0x0406, 0x0456, 0x0491, 0x00B5, 0x00B6, 0x00B7, //
    0x0451, 0x2116, 0x0454, 0x00BB, 0x0458, 0x0405, 0x0455, 0x0457,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guesses_a_cyrillic_title_as_windows_1251() {
        let title: &[u8] = &[0xCA, 0xF3, 0xEF, 0xEB, 0xE5, 0xF2];
        let encoding = guess([title, b"Guitar".as_slice()]);
        assert_eq!(encoding, Encoding::Windows1251);
        assert_eq!(decode(title, encoding), "Куплет");
        // A notice of the same kind: a space, a letter, a dot, a word.
        let notice: &[u8] = b" \xE3.\xCD\xEE\xF2\xEE\xE3\xF0\xE0\xE4";
        assert_eq!(decode(notice, encoding), " г.Нотоград");
    }

    #[test]
    fn guesses_western_accents_as_windows_1252() {
        let texts: [&[u8]; 3] = [b"Caf\xE9", b"\xC9t\xE9", b"Gr\xF6\xDFe"];
        let encoding = guess(texts);
        assert_eq!(encoding, Encoding::Windows1252);
        assert_eq!(decode(texts[1], encoding), "Été");
        assert_eq!(decode(texts[2], encoding), "Größe");
        assert_eq!(decode(b"\x93Hi\x94 \x80", encoding), "\u{201C}Hi\u{201D} €");
    }

    #[test]
    fn takes_valid_utf8_as_utf8() {
        let texts: [&[u8]; 2] = ["Тральвень".as_bytes(), "Café".as_bytes()];
        assert_eq!(guess(texts), Encoding::Utf8);
        assert_eq!(decode(texts[0], Encoding::Utf8), "Тральвень");
        assert_eq!(guess([b"plain".as_slice()]), Encoding::Windows1252);
    }

    #[test]
    fn decodes_the_rest_of_windows_1251() {
        let raw: &[u8] = &[0xA8, 0xB8, 0xB9, 0x80, 0xDF, 0xFF];
        assert_eq!(decode(raw, Encoding::Windows1251), "Ёё№ЂЯя");
    }
}
