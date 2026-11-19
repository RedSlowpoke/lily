//! W2's check (chapter 06 §9): every glyph's skyline pair from the packs'
//! outlines equals LilyPond's `ly:skylines-for-stencil` of
//! `ly:font-get-glyph`, point for point and bit for bit: the 663 drawn
//! glyphs of emmentaler-20 at the default staff size (chapter 06 §5.1), the
//! other seven sizes and the braces. The packs are built from the installed
//! LilyPond's OTFs by tools/packs/build.mjs and the skylines captured by
//! tools/packs/skylines.ly; both need LilyPond 2.26.0, and the tests skip
//! without it.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use lily_engine_fonts::Font;
use lily_engine_skyline::{Axis, Offset, glyph_skylines};

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
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("skylines");
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
            .arg(engine.join("tools/packs/skylines.ly"))
            .env("SKYLINES_OUT", &dir)
            .current_dir(&dir)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&captured.stderr);
        assert!(
            captured.status.success(),
            "lilypond failed on tools/packs/skylines.ly: {stderr}"
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

/// A number as skylines.ly writes it: an integer, `p/q` with `q` a power of
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

/// A glyph's captured skylines: its name and the points of down and up.
struct Captured {
    name: String,
    down: Vec<Offset>,
    up: Vec<Offset>,
}

/// A capture: the font, its magnification and each glyph's skylines.
fn captured(dir: &Path, font: &str) -> (f64, Vec<Captured>) {
    let text = std::fs::read_to_string(dir.join(format!("{font}.tsv"))).unwrap();
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap().split('\t').collect();
    assert_eq!(header[0], font);
    let mut glyphs: Vec<Captured> = Vec::new();
    for line in lines {
        let [name, side, numbers] = line.split('\t').collect::<Vec<_>>()[..] else {
            panic!("{font}: {line}");
        };
        let numbers: Vec<f64> = numbers.split(' ').map(exact).collect();
        let points: Vec<Offset> = numbers.chunks(2).map(|p| [p[0], p[1]]).collect();
        match side {
            "down" => glyphs.push(Captured {
                name: name.to_string(),
                down: points,
                up: Vec::new(),
            }),
            "up" => glyphs.last_mut().unwrap().up = points,
            _ => panic!("{font}: {line}"),
        }
    }
    (exact(header[1]), glyphs)
}

fn bits(points: &[Offset]) -> Vec<[u64; 2]> {
    points.iter().map(|p| p.map(f64::to_bits)).collect()
}

/// Compares every drawn glyph's skylines with the capture, in the font's
/// order; returns the number of points.
fn compare(dir: &Path, name: &str) -> usize {
    let font = load(dir, name);
    let (magnification, glyphs) = captured(dir, name);
    // ly:font-get-glyph gives an empty stencil only for .notdef, glyph 0.
    let drawn: Vec<&str> = font.glyphs()[1..].iter().map(|g| g.name.as_str()).collect();
    let theirs: Vec<&str> = glyphs.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(drawn, theirs, "{name}: the drawn glyphs and their order");
    let mut differ = Vec::new();
    let mut points = 0;
    for glyph in &glyphs {
        let (index, _) = font.find_by_name(&glyph.name).unwrap();
        let ours = glyph_skylines(&font, index, magnification, Axis::X);
        let (down, up) = (ours.down.to_points(Axis::X), ours.up.to_points(Axis::X));
        points += glyph.down.len() + glyph.up.len();
        if bits(&down) != bits(&glyph.down) || bits(&up) != bits(&glyph.up) {
            differ.push(format!(
                "{}: ours {} + {} points, LilyPond's {} + {}; first different down {:?}, up {:?}",
                glyph.name,
                down.len(),
                up.len(),
                glyph.down.len(),
                glyph.up.len(),
                first_difference(&down, &glyph.down),
                first_difference(&up, &glyph.up),
            ));
        }
    }
    assert!(
        differ.is_empty(),
        "{name}: {} of {} glyphs differ, first {:#?}",
        differ.len(),
        glyphs.len(),
        &differ[..differ.len().min(5)]
    );
    points
}

fn first_difference(ours: &[Offset], theirs: &[Offset]) -> Option<(usize, Offset, Offset)> {
    let i = (0..ours.len().max(theirs.len())).find(|&i| {
        ours.get(i).map(|p| p.map(f64::to_bits)) != theirs.get(i).map(|p| p.map(f64::to_bits))
    })?;
    let missing = [f64::NAN; 2];
    Some((
        i,
        ours.get(i).copied().unwrap_or(missing),
        theirs.get(i).copied().unwrap_or(missing),
    ))
}

/// Chapter 06 §5.1's measurement: 663 glyphs, 35,110 points.
#[test]
fn the_663_glyphs_of_emmentaler_20_have_lilypond_s_skylines() {
    let dir = need_lilypond!();
    assert_eq!(compare(dir, "emmentaler-20"), 35_110);
}

#[test]
fn every_other_size_and_the_braces_have_lilypond_s_skylines() {
    let dir = need_lilypond!();
    for name in [
        "emmentaler-11",
        "emmentaler-13",
        "emmentaler-14",
        "emmentaler-16",
        "emmentaler-18",
        "emmentaler-23",
        "emmentaler-26",
        "emmentaler-brace",
    ] {
        compare(dir, name);
    }
}
