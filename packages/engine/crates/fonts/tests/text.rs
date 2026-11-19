//! W1's text packs (chapter 06 §5.3): tools/packs/text.mjs builds the
//! twelve URW styles and TeX Gyre Schola's four (E23) from the installed
//! LilyPond's OTFs, and each URW style reads back
//! with what fontTools 4.41 and HarfBuzz 14.5 find in the OTF: the names, the
//! 854 characters, the kerning pairs, `liga`, and a few advances, pairs and
//! ink boxes, the boxes those of the outlines (D209), which come in a pack
//! of their own that checks them (D262). The tests skip without
//! LilyPond 2.26.0, whose fonts they are.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use lily_engine_fonts::{Ligature, Lookup, Outlines, Segment, TextFont, TextGlyph};

/// Each style: its OTF, family, style, weight, italic and kerning pairs.
const STYLES: [(&str, &str, &str, u32, bool, usize); 12] = [
    ("C059-Roman", "C059", "Roman", 400, false, 3821),
    ("C059-Bold", "C059", "Bold", 700, false, 3812),
    ("C059-Italic", "C059", "Italic", 400, true, 3800),
    ("C059-BdIta", "C059", "Bold Italic", 700, true, 3816),
    (
        "NimbusSans-Regular",
        "Nimbus Sans",
        "Regular",
        400,
        false,
        3838,
    ),
    ("NimbusSans-Bold", "Nimbus Sans", "Bold", 700, false, 3789),
    (
        "NimbusSans-Italic",
        "Nimbus Sans",
        "Italic",
        400,
        true,
        3870,
    ),
    (
        "NimbusSans-BoldItalic",
        "Nimbus Sans",
        "Bold Italic",
        700,
        true,
        3818,
    ),
    (
        "NimbusMonoPS-Regular",
        "Nimbus Mono PS",
        "Regular",
        400,
        false,
        0,
    ),
    ("NimbusMonoPS-Bold", "Nimbus Mono PS", "Bold", 700, false, 0),
    (
        "NimbusMonoPS-Italic",
        "Nimbus Mono PS",
        "Italic",
        400,
        true,
        0,
    ),
    (
        "NimbusMonoPS-BoldItalic",
        "Nimbus Mono PS",
        "Bold Italic",
        700,
        true,
        0,
    ),
];

/// The packs, made once for all the tests; `None` without LilyPond 2.26.0.
fn built() -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let version = Command::new("lilypond").arg("--version").output().ok()?;
        if !String::from_utf8_lossy(&version.stdout).contains("GNU LilyPond 2.26.0") {
            return None;
        }
        let engine = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("text");
        std::fs::remove_dir_all(&dir).ok();
        let packs = Command::new("node")
            .arg(engine.join("tools/packs/text.mjs"))
            .arg("--out")
            .arg(&dir)
            .output()
            .expect("node runs tools/packs/text.mjs");
        let stderr = String::from_utf8_lossy(&packs.stderr);
        assert!(
            packs.status.success(),
            "tools/packs/text.mjs failed: {stderr}"
        );
        Some(dir)
    })
    .as_deref()
}

macro_rules! need_lilypond {
    () => {
        match built() {
            Some(dir) => dir,
            None => {
                eprintln!("skipped: lilypond 2.26.0 is not installed");
                return;
            }
        }
    };
}

fn read(dir: &Path, file: &str) -> Vec<u8> {
    let file = dir.join(file);
    std::fs::read(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()))
}

/// The text pack of `style`, its outlines given.
fn load(dir: &Path, style: &str) -> TextFont {
    let mut font = TextFont::parse(&read(dir, &format!("{style}.pack")))
        .unwrap_or_else(|e| panic!("{style}: {e:?}"));
    assert!(!font.has_outlines(), "{style}");
    let outlines = read(dir, &format!("{style}.outlines.pack"));
    assert_eq!(TextFont::outlines_name(&outlines).as_deref(), Ok(style));
    // Each outline draws the ink box text.mjs wrote, or the pack is refused.
    font.add_outlines(&outlines)
        .unwrap_or_else(|e| panic!("{style}'s outlines: {e:?}"));
    assert!(font.has_outlines(), "{style}");
    font
}

#[test]
fn every_style_reads_back_whole() {
    let dir = need_lilypond!();
    for (style, family, name, weight, italic, pairs) in STYLES {
        let font = load(dir, style);
        assert_eq!(
            (font.name(), font.family(), font.style()),
            (style, family, name)
        );
        assert_eq!((font.weight(), font.italic()), (weight, italic), "{style}");
        assert_eq!(font.units_per_em(), 1000, "{style}");
        assert_eq!(font.glyphs().len(), 855, "{style}");
        // Every glyph but .notdef has a character, the last U+FB04 (ffl).
        assert_eq!(font.characters().len(), 854, "{style}");
        assert_eq!(
            font.characters().last(),
            Some(&('\u{fb04}', 232)),
            "{style}"
        );
        assert_eq!(font.pairs().count(), pairs, "{style}");
        for (first, second, value) in font.pairs() {
            assert_eq!(font.kerning(first, second), value);
        }
        // The f-ligatures in two lookups, and № from N r . in a third, which
        // Nimbus Sans Italic and Bold Italic do not have.
        let g = |c: char| font.glyph(c).unwrap();
        let liga = |glyphs: &[char], ligature: char| Ligature {
            glyphs: glyphs.iter().map(|&c| g(c)).collect(),
            ligature: g(ligature),
        };
        let mut expected = vec![
            vec![
                liga(&['f', 'f', 'i'], '\u{fb03}'),
                liga(&['f', 'i'], '\u{fb01}'),
            ],
            vec![
                liga(&['f', 'f', 'l'], '\u{fb04}'),
                liga(&['f', 'f'], '\u{fb00}'),
                liga(&['f', 'l'], '\u{fb02}'),
            ],
        ];
        if !(family == "Nimbus Sans" && italic) {
            expected.push(vec![liga(&['N', 'r', '.'], '\u{2116}')]);
        }
        let expected: Vec<Lookup> = expected.into_iter().map(Lookup::Ligature).collect();
        assert_eq!(
            font.lookups("liga"),
            expected.iter().collect::<Vec<_>>(),
            "{style}"
        );
        assert_eq!(font.features().collect::<Vec<_>>(), ["liga"], "{style}");
    }
}

/// TeX Gyre Schola's four styles (OPEN E23, MS8 step 6, D271), which
/// fontconfig gives "New Century Schoolbook": each reads back by its
/// PostScript name, and Regular's digits have C059 Roman's advances and ink,
/// as T26's time signatures found before the pack.
#[test]
fn tex_gyre_schola_reads_back() {
    let dir = need_lilypond!();
    for (style, name, weight, italic) in [
        ("TeXGyreSchola-Regular", "Regular", 400, false),
        ("TeXGyreSchola-Bold", "Bold", 700, false),
        ("TeXGyreSchola-Italic", "Italic", 400, true),
        ("TeXGyreSchola-BoldItalic", "Bold Italic", 700, true),
    ] {
        let font = load(dir, style);
        assert_eq!(
            (font.name(), font.family(), font.style()),
            (style, "TeXGyreSchola", name)
        );
        assert_eq!((font.weight(), font.italic()), (weight, italic), "{style}");
        assert_eq!(font.units_per_em(), 1000, "{style}");
        assert_eq!(font.glyphs().len(), 1090, "{style}");
    }
    let (schola, c059) = (load(dir, "TeXGyreSchola-Regular"), load(dir, "C059-Roman"));
    for digit in '0'..='9' {
        let (s, c) = (schola.glyph(digit).unwrap(), c059.glyph(digit).unwrap());
        assert_eq!(
            schola.glyphs()[s as usize],
            c059.glyphs()[c as usize],
            "{digit}"
        );
    }
}

#[test]
fn metrics_are_the_otf_s() {
    let dir = need_lilypond!();
    // fontTools' advances and kerning, HarfBuzz's extents of o.
    let roman = load(dir, "C059-Roman");
    let g = |c: char| roman.glyph(c).unwrap();
    assert_eq!((g('A'), g('V'), g(' '), g('ж')), (34, 55, 1, 686));
    assert_eq!(roman.glyphs()[34].advance, 722);
    assert_eq!(
        roman.glyphs()[1],
        TextGlyph {
            advance: 278,
            ink: [0; 4]
        }
    );
    assert_eq!(roman.glyphs()[686].advance, 761);
    assert_eq!(
        (roman.kerning(g('A'), g('V')), roman.kerning(g('T'), g('o'))),
        (-96, -27)
    );
    assert_eq!(roman.glyphs()[g('o') as usize].ink, [34, -15, 466, 481]);
    // o's outline: two contours, each of curves.
    let o: Vec<Segment> = roman.outline(g('o') as usize).collect();
    assert_eq!(
        o.iter().filter(|s| matches!(s, Segment::Move(_))).count(),
        2
    );
    assert_eq!(roman.outline(1).count(), 0);
    assert_eq!(g('\u{fb01}'), 109);
    assert_eq!(roman.glyph('\u{3042}'), None);

    let sans = load(dir, "NimbusSans-Regular");
    let g = |c: char| sans.glyph(c).unwrap();
    assert_eq!(
        (sans.glyphs()[34].advance, sans.glyphs()[686].advance),
        (667, 669)
    );
    assert_eq!(
        (sans.kerning(g('A'), g('V')), sans.kerning(g('T'), g('o'))),
        (-71, -91)
    );
    assert_eq!(sans.glyphs()[g('o') as usize].ink, [36, -23, 510, 539]);

    let mono = load(dir, "NimbusMonoPS-Regular");
    assert!(mono.glyphs()[1..].iter().all(|glyph| glyph.advance == 600));
    assert_eq!(
        mono.glyphs()[mono.glyph('o').unwrap() as usize].ink,
        [67, -16, 534, 433]
    );

    let bold_italic = load(dir, "C059-BdIta");
    let g = |c: char| bold_italic.glyph(c).unwrap();
    assert_eq!(
        (
            bold_italic.glyphs()[34].advance,
            bold_italic.glyphs()[686].advance
        ),
        (741, 1114)
    );
    assert_eq!(bold_italic.kerning(g('A'), g('V')), -87);
    assert_eq!(
        bold_italic.glyphs()[g('o') as usize].ink,
        [10, -14, 540, 478]
    );
}

#[test]
fn names_and_shaping_are_pangos() {
    let dir = need_lilypond!();
    // CFF's names, which LilyPond's glyph strings carry.
    let roman = load(dir, "C059-Roman");
    let names: Vec<&str> = [0, 1, 21, 109, 230, 231]
        .iter()
        .map(|&g| roman.glyph_name(g))
        .collect();
    assert_eq!(names, [".notdef", "space", "four", "fi", "ff", "ffi"]);
    // The glyphs Pango 1.58.2 gives "office fluff Wave" in LilyPond Serif
    // at 11 points: ffi, then fl and ff by the second lookup.
    let glyphs: Vec<u16> = roman
        .shape("office fluff Wave", &lily_engine_fonts::Features::default())
        .iter()
        .map(|s| s.glyph.unwrap())
        .collect();
    assert_eq!(
        glyphs,
        [80, 231, 68, 70, 1, 110, 86, 230, 1, 56, 66, 87, 70]
    );
}
