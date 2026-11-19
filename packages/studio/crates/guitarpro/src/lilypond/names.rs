//! Names the score is written with: LilyPond strings, pitches, keys, drum
//! names, General MIDI's instruments as `midiInstrument` knows them, and
//! variable names made from track names.

/// `text` as a LilyPond string, quotes included: backslashes and quotes
/// escaped, line breaks and other control characters made spaces.
pub fn string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.trim().chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `text` on one line, for a `%` comment.
pub fn comment(text: &str) -> String {
    text.trim()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

const SHARPS: [&str; 12] = [
    "c", "cis", "d", "dis", "e", "f", "fis", "g", "gis", "a", "ais", "b",
];
const FLATS: [&str; 12] = [
    "c", "des", "d", "ees", "e", "f", "ges", "g", "aes", "a", "bes", "b",
];

/// A MIDI pitch in LilyPond's absolute octaves (`c'` is 60), spelt with
/// flats or sharps.
pub fn pitch(midi: u8, flats: bool) -> String {
    let names = if flats { &FLATS } else { &SHARPS };
    let mut out = String::from(names[usize::from(midi % 12)]);
    let octave = i32::from(midi / 12) - 4;
    let mark = if octave > 0 { "'" } else { "," };
    for _ in 0..octave.unsigned_abs() {
        out.push_str(mark);
    }
    out
}

/// `\key`'s arguments for a key of `fifths` sharps (or flats, below 0).
pub fn key(fifths: i8, minor: bool) -> String {
    const MAJOR: [&str; 15] = [
        "ces", "ges", "des", "aes", "ees", "bes", "f", "c", "g", "d", "a", "e", "b", "fis", "cis",
    ];
    const MINOR: [&str; 15] = [
        "aes", "ees", "bes", "f", "c", "g", "d", "a", "e", "b", "fis", "cis", "gis", "dis", "ais",
    ];
    let i = usize::try_from(fifths.clamp(-7, 7) + 7).unwrap_or(7);
    if minor {
        format!("{} \\minor", MINOR[i])
    } else {
        format!("{} \\major", MAJOR[i])
    }
}

/// LilyPond's `\drummode` name for a General MIDI percussion key. Keys
/// outside GM's 35–81 get the nearest sound LilyPond has a name for, and
/// the side stick when there is none.
pub fn drum(key: u8) -> &'static str {
    match key {
        35 => "acousticbassdrum",
        36 => "bassdrum",
        37 => "sidestick",
        38 => "acousticsnare",
        39 => "handclap",
        40 => "electricsnare",
        41 => "lowfloortom",
        42 => "closedhihat",
        43 => "highfloortom",
        44 => "pedalhihat",
        45 => "lowtom",
        46 => "openhihat",
        47 => "lowmidtom",
        48 => "himidtom",
        49 => "crashcymbala",
        50 => "hightom",
        51 => "ridecymbala",
        52 => "chinesecymbal",
        53 => "ridebell",
        54 => "tambourine",
        55 => "splashcymbal",
        56 => "cowbell",
        57 => "crashcymbalb",
        58 => "vibraslap",
        59 => "ridecymbalb",
        60 => "hibongo",
        61 => "lobongo",
        62 => "mutehiconga",
        63 => "openhiconga",
        64 => "loconga",
        65 => "hitimbale",
        66 => "lotimbale",
        67 => "hiagogo",
        68 => "loagogo",
        69 => "cabasa",
        70 => "maracas",
        71 => "shortwhistle",
        72 => "longwhistle",
        73 => "shortguiro",
        74 => "longguiro",
        75 => "claves",
        76 => "hiwoodblock",
        77 => "lowoodblock",
        78 => "mutecuica",
        79 => "opencuica",
        80 => "mutetriangle",
        81 => "opentriangle",
        // GM2 and Guitar Pro's extras: shaker, jingle bell, bell tree,
        // castanets, mute and open surdo.
        82 => "maracas",
        83 => "tambourine",
        84 => "opentriangle",
        85 => "claves",
        86 => "lowtom",
        87 => "lowfloortom",
        _ => "sidestick",
    }
}

/// General MIDI's 128 programs by the names LilyPond's `midiInstrument`
/// takes (scm/midi.scm).
const INSTRUMENTS: [&str; 128] = [
    "acoustic grand",
    "bright acoustic",
    "electric grand",
    "honky-tonk",
    "electric piano 1",
    "electric piano 2",
    "harpsichord",
    "clav",
    "celesta",
    "glockenspiel",
    "music box",
    "vibraphone",
    "marimba",
    "xylophone",
    "tubular bells",
    "dulcimer",
    "drawbar organ",
    "percussive organ",
    "rock organ",
    "church organ",
    "reed organ",
    "accordion",
    "harmonica",
    "concertina",
    "acoustic guitar (nylon)",
    "acoustic guitar (steel)",
    "electric guitar (jazz)",
    "electric guitar (clean)",
    "electric guitar (muted)",
    "overdriven guitar",
    "distorted guitar",
    "guitar harmonics",
    "acoustic bass",
    "electric bass (finger)",
    "electric bass (pick)",
    "fretless bass",
    "slap bass 1",
    "slap bass 2",
    "synth bass 1",
    "synth bass 2",
    "violin",
    "viola",
    "cello",
    "contrabass",
    "tremolo strings",
    "pizzicato strings",
    "orchestral harp",
    "timpani",
    "string ensemble 1",
    "string ensemble 2",
    "synthstrings 1",
    "synthstrings 2",
    "choir aahs",
    "voice oohs",
    "synth voice",
    "orchestra hit",
    "trumpet",
    "trombone",
    "tuba",
    "muted trumpet",
    "french horn",
    "brass section",
    "synthbrass 1",
    "synthbrass 2",
    "soprano sax",
    "alto sax",
    "tenor sax",
    "baritone sax",
    "oboe",
    "english horn",
    "bassoon",
    "clarinet",
    "piccolo",
    "flute",
    "recorder",
    "pan flute",
    "blown bottle",
    "shakuhachi",
    "whistle",
    "ocarina",
    "lead 1 (square)",
    "lead 2 (sawtooth)",
    "lead 3 (calliope)",
    "lead 4 (chiff)",
    "lead 5 (charang)",
    "lead 6 (voice)",
    "lead 7 (fifths)",
    "lead 8 (bass+lead)",
    "pad 1 (new age)",
    "pad 2 (warm)",
    "pad 3 (polysynth)",
    "pad 4 (choir)",
    "pad 5 (bowed)",
    "pad 6 (metallic)",
    "pad 7 (halo)",
    "pad 8 (sweep)",
    "fx 1 (rain)",
    "fx 2 (soundtrack)",
    "fx 3 (crystal)",
    "fx 4 (atmosphere)",
    "fx 5 (brightness)",
    "fx 6 (goblins)",
    "fx 7 (echoes)",
    "fx 8 (sci-fi)",
    "sitar",
    "banjo",
    "shamisen",
    "koto",
    "kalimba",
    "bagpipe",
    "fiddle",
    "shanai",
    "tinkle bell",
    "agogo",
    "steel drums",
    "woodblock",
    "taiko drum",
    "melodic tom",
    "synth drum",
    "reverse cymbal",
    "guitar fret noise",
    "breath noise",
    "seashore",
    "bird tweet",
    "telephone ring",
    "helicopter",
    "applause",
    "gunshot",
];

/// LilyPond's `midiInstrument` for a General MIDI program (from 0).
pub fn instrument(program: u8) -> &'static str {
    INSTRUMENTS[usize::from(program & 0x7f)]
}

/// A track name as a LilyPond variable name: camelCase ASCII letters, the
/// Cyrillic transliterated, numbers spelt as words (`Acoustic 01` is
/// `acousticOne`), anything else a break between words. `track` when
/// nothing is left.
pub fn variable(name: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut digits = String::new();
    let flush_digits = |digits: &mut String, words: &mut Vec<String>| {
        if !digits.is_empty() {
            words.push(number(digits));
            digits.clear();
        }
    };
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_digit() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            digits.push(c);
            continue;
        }
        flush_digits(&mut digits, &mut words);
        if c.is_ascii_lowercase() {
            word.push(c);
        } else if let Some(latin) = cyrillic(c) {
            word.push_str(latin);
        } else if !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
    }
    flush_digits(&mut digits, &mut words);
    if !word.is_empty() {
        words.push(word);
    }
    let mut out = String::new();
    for w in words.iter().filter(|w| !w.is_empty()) {
        if out.is_empty() {
            out.push_str(&w.to_ascii_lowercase());
        } else {
            let mut chars = w.chars();
            if let Some(first) = chars.next() {
                out.push(first.to_ascii_uppercase());
                out.push_str(chars.as_str());
            }
        }
    }
    if out.is_empty() { "track".into() } else { out }
}

/// A run of digits as a capitalised English word: `01` is `One`, `21` is
/// `TwoOne`.
fn number(digits: &str) -> String {
    const WORDS: [&str; 21] = [
        "Zero",
        "One",
        "Two",
        "Three",
        "Four",
        "Five",
        "Six",
        "Seven",
        "Eight",
        "Nine",
        "Ten",
        "Eleven",
        "Twelve",
        "Thirteen",
        "Fourteen",
        "Fifteen",
        "Sixteen",
        "Seventeen",
        "Eighteen",
        "Nineteen",
        "Twenty",
    ];
    let trimmed = digits.trim_start_matches('0');
    match trimmed.parse::<usize>() {
        Ok(n) if n < WORDS.len() => WORDS[n].into(),
        _ if trimmed.is_empty() => WORDS[0].into(),
        _ => trimmed
            .bytes()
            .map(|b| WORDS[usize::from(b - b'0')])
            .collect(),
    }
}

/// Russian and Ukrainian letters in Latin, lower case.
fn cyrillic(c: char) -> Option<&'static str> {
    Some(match c {
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' | 'ґ' => "g",
        'д' => "d",
        'е' | 'ё' | 'э' => "e",
        'є' => "ye",
        'ж' => "zh",
        'з' => "z",
        'и' | 'і' => "i",
        'ї' => "yi",
        'й' | 'ы' => "y",
        'к' => "k",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'с' => "s",
        'т' => "t",
        'у' => "u",
        'ф' => "f",
        'х' => "kh",
        'ц' => "ts",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "shch",
        'ъ' | 'ь' => "",
        'ю' => "yu",
        'я' => "ya",
        _ => return None,
    })
}

/// Hands out names no other track has: the second *FX* is `fxB`, the third
/// `fxC`.
#[derive(Default)]
pub struct Namer {
    taken: Vec<String>,
}

impl Namer {
    pub fn unique(&mut self, base: &str) -> String {
        let mut n = 0usize;
        loop {
            let name = if n == 0 {
                base.to_string()
            } else {
                format!("{base}{}", letters(n))
            };
            if !self.taken.contains(&name) {
                self.taken.push(name.clone());
                return name;
            }
            n += 1;
        }
    }
}

/// 1 → `B`, 2 → `C`, …, 25 → `Z`, 26 → `BA`: letters, as a name may not
/// hold digits.
fn letters(mut n: usize) -> String {
    let mut out = Vec::new();
    loop {
        out.push(b'A' + (n % 26) as u8);
        n /= 26;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_escaped() {
        assert_eq!(string(r#"A "big" \ song"#), r#""A \"big\" \\ song""#);
        assert_eq!(string("two\nlines"), "\"two lines\"");
        assert_eq!(string("Тральвень"), "\"Тральвень\"");
    }

    #[test]
    fn pitches_are_absolute() {
        assert_eq!(pitch(60, false), "c'");
        assert_eq!(pitch(40, false), "e,");
        assert_eq!(pitch(49, false), "cis");
        assert_eq!(pitch(49, true), "des");
        assert_eq!(pitch(28, false), "e,,");
        assert_eq!(pitch(76, false), "e''");
    }

    #[test]
    fn keys_follow_the_circle_of_fifths() {
        assert_eq!(key(0, false), "c \\major");
        assert_eq!(key(1, true), "e \\minor");
        assert_eq!(key(-3, false), "ees \\major");
        assert_eq!(key(9, false), "cis \\major");
    }

    #[test]
    fn variables_are_letters_only() {
        assert_eq!(variable("Lead Guitar"), "leadGuitar");
        assert_eq!(variable("Acoustic 01"), "acousticOne");
        assert_eq!(variable("FX"), "fx");
        assert_eq!(variable("Гитара соло"), "gitaraSolo");
        assert_eq!(variable("E-Bass (5 str.)"), "eBassFiveStr");
        assert_eq!(variable("Track 123"), "trackOneTwoThree");
        assert_eq!(variable("♪ ♫"), "track");
        assert_eq!(variable(""), "track");
    }

    #[test]
    fn names_are_made_unique() {
        let mut namer = Namer::default();
        assert_eq!(namer.unique("fx"), "fx");
        assert_eq!(namer.unique("fx"), "fxB");
        assert_eq!(namer.unique("fx"), "fxC");
        assert_eq!(namer.unique("fxB"), "fxBB");
        assert_eq!(letters(26), "BA");
    }

    #[test]
    fn drums_and_instruments_have_lilypond_names() {
        assert_eq!(drum(36), "bassdrum");
        assert_eq!(drum(42), "closedhihat");
        assert_eq!(drum(127), "sidestick");
        assert_eq!(instrument(30), "distorted guitar");
        assert_eq!(instrument(33), "electric bass (finger)");
        assert_eq!(instrument(127), "gunshot");
    }
}
