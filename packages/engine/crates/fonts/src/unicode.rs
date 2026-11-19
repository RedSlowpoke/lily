//! What W6's text path (MS4 step 19) needs of Unicode's character data, as
//! LilyPond 2.26.0's Pango 1.58.2, HarfBuzz 14.4.0 and Guile 3.0 read it,
//! for the characters the engine meets:
//!
//! - [`formal_name`]: Guile's `char->formal-name`, which LilyPond's "no
//!   glyph for character" warning names a character by: from `names.rs`'
//!   table (ASCII, Latin-1, Hebrew, the CJK symbols and the kana), and by
//!   rule for Hangul syllables and CJK compatibility ideographs. Guile names
//!   no CJK unified ideograph; any other character has no name here.
//! - [`script`]: `g_unichar_get_script` for the scripts a score's text
//!   reaches, which Pango's runs are split by; any other character is
//!   [`Script::Common`]. [`paired`]: `pango-script.c`'s paired characters.
//! - [`keeps_font`]: the characters Pango's itemizing selects no font for
//!   (spaces, controls, format characters, variation selectors).
//! - [`space_fallback`]: HarfBuzz's space fallback type of a space character
//!   a font has not, which it draws with the font's space at its own width.
use alloc::string::String;

use crate::names::{BLOCKS, PREFIXES};

/// Guile's `char->formal-name` of `c`, `None` where it gives `#f` or the
/// engine does not hold the name.
pub fn formal_name(c: char) -> Option<String> {
    let u = c as u32;
    if (0xAC00..=0xD7A3).contains(&u) {
        return Some(hangul(u - 0xAC00));
    }
    if (0xF900..=0xFA6D).contains(&u)
        || (0xFA70..=0xFAD9).contains(&u)
        || (0x2F800..=0x2FA1D).contains(&u)
    {
        return Some(alloc::format!("CJK COMPATIBILITY IDEOGRAPH-{u:X}"));
    }
    let &(first, _, names) = BLOCKS.iter().find(|(a, b, _)| (*a..=*b).contains(&u))?;
    let packed = names.split('|').nth((u - first) as usize + 1)?;
    let mut name = String::new();
    for ch in packed.chars() {
        match PREFIXES.get((ch as usize).wrapping_sub(1)) {
            Some(prefix) => name.push_str(prefix),
            None => name.push(ch),
        }
    }
    (!name.is_empty()).then_some(name)
}

/// Unicode's rule for a Hangul syllable's name (§3.12), by its index from
/// U+AC00, with libunistring's final jamo NI where Unicode's is NJ, as
/// Guile names U+AC05 `HANGUL SYLLABLE GANI`.
fn hangul(s: u32) -> String {
    const L: [&str; 19] = [
        "G", "GG", "N", "D", "DD", "R", "M", "B", "BB", "S", "SS", "", "J", "JJ", "C", "K", "T",
        "P", "H",
    ];
    const V: [&str; 21] = [
        "A", "AE", "YA", "YAE", "EO", "E", "YEO", "YE", "O", "WA", "WAE", "OE", "YO", "U", "WEO",
        "WE", "WI", "YU", "EU", "YI", "I",
    ];
    const T: [&str; 28] = [
        "", "G", "GG", "GS", "N", "NI", "NH", "D", "L", "LG", "LM", "LB", "LS", "LT", "LP", "LH",
        "M", "B", "BS", "S", "SS", "NG", "J", "C", "K", "T", "P", "H",
    ];
    let (l, v, t) = (s / (21 * 28), s % (21 * 28) / 28, s % 28);
    alloc::format!(
        "HANGUL SYLLABLE {}{}{}",
        L[l as usize],
        V[v as usize],
        T[t as usize]
    )
}

/// A `GUnicodeScript` of [`script`]'s; `Common` and `Inherited` take their
/// neighbours' in a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    Common,
    Inherited,
    Latin,
    Greek,
    Cyrillic,
    Armenian,
    Hebrew,
    Arabic,
    Hangul,
    Hiragana,
    Katakana,
    Han,
}

impl Script {
    /// Common or Inherited: no script of its own.
    pub fn is_weak(self) -> bool {
        matches!(self, Script::Common | Script::Inherited)
    }
}

/// Where [`script`] gives other than `Common`: first, last, script, ascending.
const SCRIPTS: [(u32, u32, Script); 62] = {
    use Script::*;
    [
        (0x41, 0x5A, Latin),
        (0x61, 0x7A, Latin),
        (0xAA, 0xAA, Latin),
        (0xBA, 0xBA, Latin),
        (0xC0, 0xD6, Latin),
        (0xD8, 0xF6, Latin),
        (0xF8, 0x2B8, Latin),
        (0x2E0, 0x2E4, Latin),
        (0x300, 0x36F, Inherited),
        (0x370, 0x373, Greek),
        (0x375, 0x377, Greek),
        (0x37A, 0x37D, Greek),
        (0x37F, 0x383, Greek),
        (0x384, 0x384, Greek),
        (0x386, 0x386, Greek),
        (0x388, 0x3E1, Greek),
        (0x3F0, 0x3FF, Greek),
        (0x400, 0x484, Cyrillic),
        (0x485, 0x486, Inherited),
        (0x487, 0x52F, Cyrillic),
        (0x531, 0x58A, Armenian),
        (0x58D, 0x58F, Armenian),
        (0x591, 0x5F4, Hebrew),
        (0x600, 0x604, Arabic),
        (0x606, 0x60B, Arabic),
        (0x60D, 0x61A, Arabic),
        (0x61C, 0x61E, Arabic),
        (0x620, 0x63F, Arabic),
        (0x641, 0x64A, Arabic),
        (0x64B, 0x655, Inherited),
        (0x656, 0x66F, Arabic),
        (0x670, 0x670, Inherited),
        (0x671, 0x6DC, Arabic),
        (0x6DE, 0x6FF, Arabic),
        (0x1100, 0x11FF, Hangul),
        (0x1AB0, 0x1AFF, Inherited),
        (0x1C80, 0x1C8A, Cyrillic),
        (0x1D00, 0x1D25, Latin),
        (0x1DC0, 0x1DFF, Inherited),
        (0x1E00, 0x1EFF, Latin),
        (0x1F00, 0x1FFE, Greek),
        (0x200C, 0x200D, Inherited),
        (0x20D0, 0x20F0, Inherited),
        (0x2C60, 0x2C7F, Latin),
        (0x2DE0, 0x2DFF, Cyrillic),
        (0x2E80, 0x2FD5, Han),
        (0x3005, 0x3005, Han),
        (0x3007, 0x3007, Han),
        (0x3021, 0x3029, Han),
        (0x302A, 0x302D, Inherited),
        (0x3038, 0x303B, Han),
        (0x3041, 0x3096, Hiragana),
        (0x3099, 0x309A, Inherited),
        (0x309D, 0x309F, Hiragana),
        (0x30A1, 0x30FA, Katakana),
        (0x30FD, 0x30FF, Katakana),
        (0x3131, 0x318E, Hangul),
        (0x31F0, 0x31FF, Katakana),
        (0x3400, 0x4DBF, Han),
        (0x4E00, 0x9FFF, Han),
        (0xAC00, 0xD7A3, Hangul),
        (0xF900, 0xFAFF, Han),
    ]
};

/// `g_unichar_get_script` of `c`, for the scripts of [`Script`]; any other
/// character is `Common`.
pub fn script(c: char) -> Script {
    let u = c as u32;
    let at = SCRIPTS.partition_point(|&(_, last, _)| last < u);
    match SCRIPTS.get(at) {
        Some(&(first, _, s)) if first <= u => s,
        _ if (0xFE00..=0xFE0F).contains(&u) || (0xFE20..=0xFE2F).contains(&u) => Script::Inherited,
        _ if (0x20000..=0x3FFFF).contains(&u) => Script::Han,
        _ => Script::Common,
    }
}

/// `pango-script.c`'s `paired_chars`, ascending: each opening character
/// before its closing one.
const PAIRED: [u32; 34] = [
    0x28, 0x29, 0x3C, 0x3E, 0x5B, 0x5D, 0x7B, 0x7D, 0xAB, 0xBB, 0x2018, 0x2019, 0x201C, 0x201D,
    0x2039, 0x203A, 0x3008, 0x3009, 0x300A, 0x300B, 0x300C, 0x300D, 0x300E, 0x300F, 0x3010, 0x3011,
    0x3014, 0x3015, 0x3016, 0x3017, 0x3018, 0x3019, 0x301A, 0x301B,
];

/// The pair of a paired character and whether it opens it.
pub fn paired(c: char) -> Option<(usize, bool)> {
    let at = PAIRED.binary_search(&(c as u32)).ok()?;
    Some((at / 2, at % 2 == 0))
}

/// Whether Pango's itemizing gives `c` no font of its own, so that it takes
/// its neighbours': a space separator but U+1680, a control, a format
/// character, a line or paragraph separator, a variation selector.
pub fn keeps_font(c: char) -> bool {
    matches!(c as u32,
        0..=0x20 | 0x7F..=0xA0 | 0xAD | 0x600..=0x605 | 0x61C | 0x6DD | 0x70F | 0x180E
        | 0x2000..=0x200F | 0x2028..=0x202F | 0x205F..=0x2064 | 0x2066..=0x206F | 0x3000
        | 0xFE00..=0xFE0F | 0xFEFF | 0xFFF9..=0xFFFB | 0xE0001 | 0xE0020..=0xE007F
        | 0xE0100..=0xE01EF)
}

/// HarfBuzz's `space_t` of a space character it falls back for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    /// The space's own width: U+0020, U+00A0.
    Space,
    /// An em divided by this: U+2001, U+2003, U+3000 (1), U+2000, U+2002
    /// (2), U+2004 (3), U+2005 (4), U+2009 (5), U+2006 (6), U+200A (16).
    Em(i64),
    /// Four eighteenths of an em: U+205F.
    FourEm18,
    /// A digit's width: U+2007.
    Figure,
    /// A full stop's, or a comma's: U+2008.
    Punctuation,
    /// Half the space's: U+202F.
    Narrow,
}

/// `hb_unicode_funcs_t::space_fallback_type`.
pub fn space_fallback(c: char) -> Option<Space> {
    Some(match c as u32 {
        0x20 | 0xA0 => Space::Space,
        0x2001 | 0x2003 | 0x3000 => Space::Em(1),
        0x2000 | 0x2002 => Space::Em(2),
        0x2004 => Space::Em(3),
        0x2005 => Space::Em(4),
        0x2009 => Space::Em(5),
        0x2006 => Space::Em(6),
        0x200A => Space::Em(16),
        0x205F => Space::FourEm18,
        0x2007 => Space::Figure,
        0x2008 => Space::Punctuation,
        0x202F => Space::Narrow,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_guiles() {
        let name = |c| formal_name(c).unwrap_or_default();
        assert_eq!(name(' '), "SPACE");
        assert_eq!(name('é'), "LATIN SMALL LETTER E WITH ACUTE");
        assert_eq!(name('א'), "HEBREW LETTER ALEF");
        assert_eq!(name('ぁ'), "HIRAGANA LETTER SMALL A");
        assert_eq!(name('\u{3000}'), "IDEOGRAPHIC SPACE");
        assert_eq!(name('\u{AC00}'), "HANGUL SYLLABLE GA");
        assert_eq!(name('\u{D7A3}'), "HANGUL SYLLABLE HIH");
        assert_eq!(name('\u{F900}'), "CJK COMPATIBILITY IDEOGRAPH-F900");
        // Guile names no CJK unified ideograph, nor an unassigned Hebrew character.
        assert_eq!(formal_name('一'), None);
        assert_eq!(formal_name('\u{5C8}'), None);
        assert_eq!(formal_name('\u{FA6E}'), None);
        // Beyond the table.
        assert_eq!(formal_name('\u{2009}'), None);
    }

    #[test]
    fn scripts_and_pairs() {
        assert_eq!(script('a'), Script::Latin);
        assert_eq!(script('é'), Script::Latin);
        assert_eq!(script('×'), Script::Common);
        assert_eq!(script('Ж'), Script::Cyrillic);
        assert_eq!(script('\u{301}'), Script::Inherited);
        assert_eq!(script('א'), Script::Hebrew);
        assert_eq!(script('あ'), Script::Hiragana);
        assert_eq!(script('ー'), Script::Common);
        assert_eq!(script('一'), Script::Han);
        assert_eq!(script('1'), Script::Common);
        assert_eq!(script('\u{2009}'), Script::Common);
        assert_eq!(paired('('), Some((0, true)));
        assert_eq!(paired(')'), Some((0, false)));
        assert_eq!(paired('」'), Some((10, false)));
        assert_eq!(paired('a'), None);
        assert!(keeps_font(' ') && keeps_font('\u{2009}') && keeps_font('\u{AD}'));
        assert!(!keeps_font('a') && !keeps_font('\u{1680}'));
        assert_eq!(space_fallback('\u{2009}'), Some(Space::Em(5)));
        assert_eq!(space_fallback('a'), None);
    }
}
