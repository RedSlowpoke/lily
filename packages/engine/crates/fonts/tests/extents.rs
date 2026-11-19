//! W1's check (chapter 06 §9): every glyph's extent from the packs equals
//! LilyPond's `ly:stencil-extent` of `ly:font-get-glyph`, bit for bit, for the
//! eight optical sizes and the braces, and for the fonts the layout gets at
//! the default staff size once scaled by their magnification. The packs are
//! built from the installed LilyPond's OTFs by tools/packs/build.mjs and the
//! extents captured by tools/packs/extents.ly; both need LilyPond 2.26.0, and
//! the tests skip without it. Emmentaler's text is shaped with the `kern`
//! and GSUB features its packs hold (W6, D209), into the glyphs LilyPond's
//! markups give.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use lily_engine_fonts::{Box, Features, Font, Interval, TextFont};

const SIZES: [u32; 8] = [11, 13, 14, 16, 18, 20, 23, 26];
const BRACE_PARTS: usize = 4;

/// The packs and the captures, made once for all the tests; `None` without LilyPond 2.26.0.
fn built() -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let version = Command::new("lilypond").arg("--version").output().ok()?;
        if !String::from_utf8_lossy(&version.stdout).contains("GNU LilyPond 2.26.0") {
            return None;
        }
        let engine = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("fonts");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let packs = Command::new("node")
            .arg(engine.join("tools/packs/build.mjs"))
            .arg("--out")
            .arg(&dir)
            .output()
            .expect("node runs tools/packs/build.mjs");
        let stderr = String::from_utf8_lossy(&packs.stderr);
        assert!(
            packs.status.success(),
            "tools/packs/build.mjs failed: {stderr}"
        );
        let captured = Command::new("lilypond")
            .args(["-s", "-dno-print-pages"])
            .arg(engine.join("tools/packs/extents.ly"))
            .env("EXTENTS_OUT", &dir)
            .current_dir(&dir)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&captured.stderr);
        assert!(
            captured.status.success(),
            "lilypond failed on tools/packs/extents.ly: {stderr}"
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

fn load(dir: &Path, font: &str) -> Font {
    let read =
        |file: String| std::fs::read(dir.join(&file)).unwrap_or_else(|e| panic!("{file}: {e}"));
    if font != "emmentaler-brace" {
        return Font::parse(&read(format!("{font}.pack"))).unwrap();
    }
    let mut braces = Font::parse(&read(format!("{font}-0.pack"))).unwrap();
    for part in 1..BRACE_PARTS {
        braces.add(&read(format!("{font}-{part}.pack"))).unwrap();
    }
    braces
}

/// A number as extents.ly writes it: an integer, `p/q` with `q` a power of
/// two (so both convert exactly), `-0`, or an infinity.
fn exact(text: &str) -> f64 {
    match text {
        "-0" => -0.0,
        "+inf.0" => f64::INFINITY,
        "-inf.0" => f64::NEG_INFINITY,
        _ => match text.split_once('/') {
            None => text.parse::<i64>().unwrap() as f64,
            Some((p, q)) => {
                let (p, q) = (p.parse::<i64>().unwrap(), q.parse::<u128>().unwrap());
                assert!(p.unsigned_abs() < 1 << 53 && q.is_power_of_two(), "{text}");
                p as f64 / q as f64
            }
        },
    }
}

/// A capture's lines: the glyph's name and its extent.
fn captured(dir: &Path, file: &str) -> (Vec<String>, Vec<(String, Box)>) {
    let text = std::fs::read_to_string(dir.join(file)).unwrap();
    let mut header = Vec::new();
    let mut glyphs = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 5 {
            header.extend(fields.iter().map(|f| f.to_string()));
            continue;
        }
        let [x0, x1, y0, y1] = [1, 2, 3, 4].map(|i| exact(fields[i]));
        glyphs.push((
            fields[0].to_string(),
            Box {
                x: Interval::new(x0, x1),
                y: Interval::new(y0, y1),
            },
        ));
    }
    (header, glyphs)
}

fn bits(b: Box) -> [u64; 4] {
    [b.x.left, b.x.right, b.y.left, b.y.right].map(f64::to_bits)
}

/// Compares every glyph of `font`, its extents scaled by `scale`, with the
/// capture, in the capture's order, which is the font's.
fn compare(font: &Font, glyphs: &[(String, Box)], scale: Option<f64>) {
    let names: Vec<&str> = font.glyphs().iter().map(|g| g.name.as_str()).collect();
    let theirs: Vec<&str> = glyphs.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, theirs, "{}: the glyphs and their order", font.name());
    let differ: Vec<String> = glyphs
        .iter()
        .filter_map(|(name, extent)| {
            let ours = font
                .find_by_name(name)
                .map_or(Box::EMPTY, |(_, b)| scale.map_or(b, |s| b.scaled(s)));
            (bits(ours) != bits(*extent))
                .then(|| format!("{name}: ours {ours:?}, LilyPond's {extent:?}"))
        })
        .collect();
    assert!(
        differ.is_empty(),
        "{}: {} of {} extents differ, first {:#?}",
        font.name(),
        differ.len(),
        glyphs.len(),
        &differ[..differ.len().min(5)]
    );
}

#[test]
fn every_glyph_of_the_eight_sizes_has_lilypond_s_extent() {
    let dir = need_lilypond!();
    for size in SIZES {
        let name = format!("emmentaler-{size}");
        let font = load(dir, &name);
        assert_eq!(font.name(), name);
        assert_eq!(font.glyphs().len(), 664);
        compare(&font, &captured(dir, &format!("{name}.tsv")).1, None);
    }
}

#[test]
fn every_brace_of_the_four_parts_has_lilypond_s_extent() {
    let dir = need_lilypond!();
    let braces = load(dir, "emmentaler-brace");
    assert_eq!(braces.glyphs().len(), 577);
    compare(&braces, &captured(dir, "emmentaler-brace.tsv").1, None);
}

/// The fonts of `ly:paper-get-font` at staff size 20: emmentaler-20,
/// emmentaler-14 at font-size -3, and the braces, each a font scaled by its
/// magnification (`Modified_font_metric`).
#[test]
fn the_staff_s_fonts_are_the_packs_scaled_by_their_magnification() {
    let dir = need_lilypond!();
    for (file, font) in [
        ("staff-fetaMusic0.tsv", "emmentaler-20"),
        ("staff-fetaMusic-3.tsv", "emmentaler-14"),
        ("staff-fetaBraces0.tsv", "emmentaler-brace"),
    ] {
        let (header, glyphs) = captured(dir, file);
        assert_eq!(header[0], font, "{file}");
        compare(&load(dir, font), &glyphs, Some(exact(&header[1])));
    }
}

/// `LILY`'s values as Guile reads them, and the one glyph of each size that
/// LILC leaves out, whose extent is FreeType's.
#[test]
fn lily_s_values_and_freetype_s_glyph_are_read() {
    let dir = need_lilypond!();
    let font = load(dir, "emmentaler-20");
    assert_eq!(font.global("staff_space"), Some(5.0));
    assert_eq!(font.global("blot_diameter"), Some(0.4));
    assert_eq!(font.global("stafflinethickness"), Some(0.50005));
    let backslash = font.name_to_index("backslash").unwrap();
    assert_eq!(font.glyphs()[backslash].metrics, None);
    let flat = font.find_by_name("accidentals.flat").unwrap().1;
    assert!(flat.x.left < flat.x.right && flat.y.left < flat.y.right);
    assert_eq!(font.find_by_name("rests.-3"), font.find_by_name("rests.M3"));
}

#[test]
fn emmentaler_s_text_is_shaped_with_its_kerning_and_features() {
    let dir = need_lilypond!();
    let font = TextFont::from_music(&load(dir, "emmentaler-20"));
    assert_eq!(
        font.features().collect::<Vec<_>>(),
        ["cv47", "dlig", "ss01", "tnum"]
    );
    let shaped = |text: &str, on: &[&str]| {
        let mut features = Features::default();
        for tag in on {
            features.set(tag, true);
        }
        font.shape(text, &features)
            .iter()
            .map(|s| (font.glyph_name(s.glyph.unwrap()), s.kerning))
            .collect::<Vec<_>>()
    };
    // The glyphs and kerning of LilyPond 2.26.0's glyph strings: \dynamic's
    // ff and mf, \volta-number's and \finger's digits (cv47, ss01), and
    // the four features at once, which dlig, the font's first lookup, finds
    // nothing of before the others substitute.
    assert_eq!(shaped("ff", &[]), [("f", -38), ("f", 0)]);
    assert_eq!(shaped("mf", &[]), [("m", -29), ("f", 0)]);
    assert_eq!(shaped("10", &[]), [("one", -25), ("zero", 0)]);
    assert_eq!(
        shaped("1.", &["cv47", "ss01"]),
        [("fattened.one", 25), ("period", 0)]
    );
    assert_eq!(
        shaped("1/2", &["dlig", "tnum", "cv47", "ss01"])
            .iter()
            .map(|&(name, _)| name)
            .collect::<Vec<_>>(),
        [
            "fattened.fixedwidth.one",
            "slash",
            "fattened.fixedwidth.two"
        ]
    );
    let mut unkerned = Features::default();
    unkerned.set("kern", false);
    assert_eq!(font.shape("ff", &unkerned)[0].kerning, 0);
    // A brace part holds no layout.
    assert_eq!(
        TextFont::from_music(&load(dir, "emmentaler-brace"))
            .features()
            .count(),
        0
    );
}
