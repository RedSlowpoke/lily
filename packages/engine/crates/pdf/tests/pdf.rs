//! W7's PDF writer natively (MS8 step 5, D270), without the VM:
//!
//! - The display list's bytes give back the list, every op of it, and
//!   bytes that are none are refused.
//! - A page of each primitive, a music glyph and a text run written by
//!   hand: the file's cross-reference table points at each object, every
//!   stream inflates, the page is the one page with its size, the glyph a
//!   form XObject drawn by `Do`, and the run's glyph ids and ToUnicode map
//!   are the run's.
//! - The 12 URW fonts of the installed LilyPond 2.26.0's `fonts/otf` (whose
//!   licence the PDF exception covers, D270) and TeX Gyre Schola's 4 (the
//!   GUST Font License, D271): none ends a glyph with
//!   `seac`, and each subset to a few glyphs reads again with every glyph
//!   kept at its id, the kept charstrings the font's own, the others one
//!   `endchar`, and fewer subroutines. Skipped without LilyPond 2.26.0, and
//!   then unverified; tools/compare/test/pdf.test.mjs checks the engine's
//!   own PDFs against its SVG pages with pdf.js.
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

use lily_engine_pdf::cff::{Cff, OpenType};
use lily_engine_pdf::{Command as Path, Document, Font, Op, Page, Segment, TextGlyph, write};

/// The URW styles LilyPond 2.26.0 ships.
const URW: [&str; 12] = [
    "C059-Roman",
    "C059-Bold",
    "C059-Italic",
    "C059-BdIta",
    "NimbusSans-Regular",
    "NimbusSans-Bold",
    "NimbusSans-Italic",
    "NimbusSans-BoldItalic",
    "NimbusMonoPS-Regular",
    "NimbusMonoPS-Bold",
    "NimbusMonoPS-Italic",
    "NimbusMonoPS-BoldItalic",
];

/// TeX Gyre Schola, which "New Century Schoolbook" is (E23), its OTFs as
/// LilyPond 2.26.0 names them; its PDFs embed it (the operator's answer at
/// MS8 step 6, D271).
const SCHOLA: [&str; 4] = [
    "texgyreschola-regular",
    "texgyreschola-bold",
    "texgyreschola-italic",
    "texgyreschola-bolditalic",
];

/// `fonts/otf` of the installed LilyPond 2.26.0, if there is one.
fn otf_dir() -> Option<PathBuf> {
    let version = Command::new("lilypond").arg("--version").output().ok()?;
    if !String::from_utf8_lossy(&version.stdout).contains("GNU LilyPond 2.26.0") {
        return None;
    }
    let path = Command::new("sh")
        .args(["-c", "command -v lilypond"])
        .output()
        .ok()?;
    let binary = std::fs::canonicalize(String::from_utf8(path.stdout).ok()?.trim()).ok()?;
    let dir = binary.parent()?.join("../share/lilypond/2.26.0/fonts/otf");
    dir.join("C059-Roman.otf").exists().then_some(dir)
}

fn every_op() -> Vec<Op> {
    vec![
        Op::Save,
        Op::Color([1.0, 0.0, 0.5]),
        Op::Transform([0.0, 1.0, -1.0, 0.0, 2.0, 3.0]),
        Op::Box {
            x: 1.0,
            y: -2.0,
            w: 3.0,
            h: 0.5,
            blot: 0.2,
        },
        Op::Box {
            x: 1.0,
            y: -4.0,
            w: 3.0,
            h: 0.5,
            blot: 0.0,
        },
        Op::Restore,
        Op::Line {
            thick: 0.1,
            from: (0.0, 0.0),
            to: (5.0, 1.0),
            dash: None,
        },
        Op::Line {
            thick: 0.1,
            from: (0.0, 1.0),
            to: (5.0, 2.0),
            dash: Some([0.3, 0.2, 0.0]),
        },
        Op::Ellipse {
            at: (3.0, -6.0),
            rx: 1.0,
            ry: 0.5,
            thick: 0.1,
            fill: false,
        },
        Op::Arc {
            at: (6.0, -6.0),
            rx: 1.0,
            ry: 1.0,
            start: 0.0,
            end: 180.0,
            thick: 0.1,
            connect: true,
            fill: true,
        },
        Op::Polygon {
            points: vec![(0.0, -8.0), (1.0, -9.0), (2.0, -8.0)],
            blot: 0.1,
            fill: true,
        },
        Op::Path {
            thick: 0.15,
            cap: 0,
            join: 2,
            fill: false,
            commands: vec![
                Path::Move(0.0, -10.0),
                Path::Curve([1.0, -9.0, 2.0, -11.0, 3.0, -10.0]),
                Path::Line(4.0, -10.0),
                Path::Close,
            ],
        },
        Op::Glyph {
            glyph: 0,
            at: (8.0, -2.0),
            scale: 0.007,
        },
        Op::Glyph {
            glyph: 0,
            at: (9.0, -2.0),
            scale: 0.007,
        },
        Op::Text {
            font: 0,
            size: 2.2,
            at: (1.0, -14.0),
            glyphs: vec![
                TextGlyph {
                    gid: 66,
                    advance: 1.1,
                    offset: (0.0, 0.0),
                    text: "a".into(),
                },
                TextGlyph {
                    gid: 67,
                    advance: 1.2,
                    offset: (0.0, 0.3),
                    text: "b".into(),
                },
            ],
        },
        Op::Link {
            rect: [0.0, 0.0, 1.0, 1.0],
            url: "https://lilypond.org/".into(),
        },
    ]
}

fn document() -> Document {
    Document {
        info: vec![
            ("Title".into(), "A (test)".into()),
            ("Composer".into(), "Dvořák".into()),
        ],
        glyphs: vec![vec![
            Segment::Move([0, -46]),
            Segment::Line([91, 116]),
            Segment::Curve([100, 0], [50, -50], [0, -46]),
        ]],
        fonts: vec!["C059-Roman".into()],
        pages: vec![Page {
            width: 595.28,
            height: 841.89,
            scale: 4.9813,
            origin: (-1.0, -168.0),
            ops: every_op(),
        }],
    }
}

#[test]
fn display_lists_give_back_what_they_hold() {
    let doc = document();
    let bytes = doc.encode();
    assert_eq!(Document::decode(&bytes).unwrap(), doc);
    assert!(Document::decode(&bytes[..bytes.len() - 1]).is_err());
    assert!(Document::decode(b"LILD\x02").is_err());
    let mut longer = bytes.clone();
    longer.push(0);
    assert!(Document::decode(&longer).is_err());
}

/// Each object of `pdf` by number, its stream inflated.
fn objects(pdf: &[u8]) -> Vec<(usize, String, Option<Vec<u8>>)> {
    let xref = pdf.windows(10).rposition(|w| w == b"startxref\n").unwrap();
    let tail = std::str::from_utf8(&pdf[xref + 10..]).unwrap();
    let at: usize = tail.lines().next().unwrap().parse().unwrap();
    let text = std::str::from_utf8(&pdf[at..]).unwrap();
    assert!(text.starts_with("xref\n0 "));
    let mut lines = text.lines().skip(1);
    let size: usize = lines
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let mut out = Vec::new();
    for (id, line) in lines.take(size).enumerate().skip(1) {
        let offset: usize = line[..10].parse().unwrap();
        assert!(line.ends_with(" n "), "object {id} is in the file");
        let head = format!("{id} 0 obj\n");
        assert_eq!(
            &pdf[offset..offset + head.len()],
            head.as_bytes(),
            "object {id}'s offset"
        );
        let body = &pdf[offset + head.len()..];
        let end = body.windows(7).position(|w| w == b"endobj\n").unwrap();
        let body = &body[..end];
        let stream = body
            .windows(8)
            .position(|w| w == b"stream\n<" || w == b"\nstream\n");
        let (dict, data) = match body.windows(8).position(|w| w == b"\nstream\n") {
            Some(s) => {
                let dict = String::from_utf8_lossy(&body[..s]).into_owned();
                let length: usize = dict
                    .split("/Length ")
                    .nth(1)
                    .unwrap()
                    .split(' ')
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap();
                let raw = &body[s + 8..s + 8 + length];
                (
                    dict,
                    Some(
                        miniz_oxide::inflate::decompress_to_vec_zlib(raw)
                            .expect("a stream inflates"),
                    ),
                )
            }
            None => (String::from_utf8_lossy(body).into_owned(), None),
        };
        let _ = stream;
        out.push((id, dict, data));
    }
    out
}

#[test]
fn a_page_of_each_primitive_is_one_page_of_its_objects() {
    let otf = otf_dir().map(|d| std::fs::read(d.join("C059-Roman.otf")).unwrap());
    let fonts: Vec<Font<'_>> = otf
        .iter()
        .map(|otf| Font {
            name: "C059-Roman",
            otf,
        })
        .collect();
    let written = write(&document(), &fonts);
    let pdf = &written.pdf;
    assert!(pdf.starts_with(b"%PDF-1.7\n"));
    assert!(pdf.ends_with(b"%%EOF\n"));
    let objects = objects(pdf);
    let pages: Vec<_> = objects
        .iter()
        .filter(|(_, d, _)| d.contains("/Type /Page "))
        .collect();
    assert_eq!(pages.len(), 1);
    assert!(pages[0].1.contains("/MediaBox [0 0 595.28 841.89]"));
    assert!(pages[0].1.contains("/URI (https://lilypond.org/)"));
    let info = &objects[2].1;
    assert!(info.contains("/Title (A \\(test\\))"), "{info}");
    assert!(
        info.contains("/Composer <FEFF00440076006F015900E1006B>"),
        "{info}"
    );
    let forms: Vec<_> = objects
        .iter()
        .filter(|(_, d, _)| d.contains("/Subtype /Form"))
        .collect();
    assert_eq!(forms.len(), 1, "the glyph once");
    assert!(forms[0].1.contains("/BBox [0 -50 100 116]"));
    assert_eq!(
        String::from_utf8_lossy(forms[0].2.as_ref().unwrap()),
        "0 -46 m\n91 116 l\n100 0 50 -50 0 -46 c\nh\nf\n"
    );
    let content = String::from_utf8(
        pages
            .iter()
            .map(|p| p.0 + 1)
            .map(|c| objects[c - 1].2.clone().unwrap())
            .next()
            .unwrap(),
    )
    .unwrap();
    assert!(
        content.starts_with("4.9813 0 0 4.9813 4.9813 836.8584 cm\n1 J 1 j\n"),
        "{content}"
    );
    assert_eq!(content.matches("/G0 Do").count(), 2);
    assert!(content.contains("[0.3 0.2] 0 d\n"));
    assert!(
        content.contains("1.1 -1.9 2.8 0.3 re\n"),
        "the box inside half its blot"
    );
    match otf {
        Some(_) => {
            assert!(written.warnings.is_empty(), "{:?}", written.warnings);
            assert!(
                content.contains("/F0 2.2 Tf\n1 0 0 1 1 -14 Tm\n[<0042>"),
                "{content}"
            );
            assert!(
                content.contains("1 0 0 1 2.1 -13.7 Tm\n[<0043>"),
                "an offset glyph placed again: {content}"
            );
            let cmap = objects
                .iter()
                .find_map(|(_, _, d)| d.as_ref().filter(|d| d.starts_with(b"/CIDInit")))
                .unwrap();
            let cmap = String::from_utf8_lossy(cmap);
            assert!(
                cmap.contains("2 beginbfchar\n<0042> <0061>\n<0043> <0062>\nendbfchar"),
                "{cmap}"
            );
            let font = objects
                .iter()
                .find(|(_, d, _)| d.contains("/Subtype /CIDFontType0 "))
                .unwrap();
            assert!(font.1.contains("/W [66 ["), "{}", font.1);
            let file = objects
                .iter()
                .find(|(_, d, _)| d.contains("/Subtype /CIDFontType0C"))
                .unwrap();
            let subset = file.2.as_ref().unwrap();
            let cff = Cff::new(subset).unwrap();
            assert!(cff.charstring(66).unwrap().len() > 1 && cff.charstring(68).unwrap() == [14]);
        }
        None => {
            assert_eq!(
                written.warnings,
                ["pdf: no font C059-Roman: its text is left out"]
            );
            assert!(!content.contains("BT"));
            eprintln!("skipped: the text half, without LilyPond 2.26.0's fonts/otf");
        }
    }
}

#[test]
fn the_text_fonts_subset_to_what_their_text_draws() {
    let Some(dir) = otf_dir() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed");
        return;
    };
    for style in URW.into_iter().chain(SCHOLA) {
        let otf = std::fs::read(dir.join(format!("{style}.otf"))).unwrap();
        let font = OpenType::new(&otf).unwrap();
        let metrics = font.metrics().unwrap();
        assert_eq!(metrics.units_per_em, 1000, "{style}");
        assert_eq!(
            metrics.fixed_pitch,
            style.starts_with("NimbusMono"),
            "{style}"
        );
        assert_eq!(
            metrics.italic_angle != 0.0,
            style.to_lowercase().contains("ital") || style.contains("BdIta"),
            "{style}"
        );
        let table = font.table(b"CFF ").unwrap();
        let cff = Cff::new(table).unwrap();
        assert!(
            !cff.has_seac().unwrap(),
            "{style}: no glyph is a seac accent"
        );
        let keep: BTreeSet<u16> = [3, 36, 37, 68, 69, 70, 200].into_iter().collect();
        let subset = cff.subset(&keep).unwrap();
        let again = Cff::new(&subset).unwrap();
        assert_eq!(again.glyph_count(), cff.glyph_count(), "{style}");
        for gid in 0..cff.glyph_count() {
            let kept = gid == 0 || keep.contains(&(gid as u16));
            let want = if kept {
                cff.charstring(gid).unwrap()
            } else {
                &[14][..]
            };
            assert_eq!(again.charstring(gid).unwrap(), want, "{style} glyph {gid}");
        }
        let (before, after) = (cff.subroutines(), again.subroutines());
        assert!(
            after.0 <= before.0 && after.1 < before.1.max(1),
            "{style}: {before:?} to {after:?}"
        );
        // A subset of every glyph keeps every subroutine a glyph calls and the font whole but for
        // those no glyph calls.
        let all: BTreeSet<u16> = (0..cff.glyph_count() as u16).collect();
        let whole = cff.subset(&all).unwrap();
        let whole = Cff::new(&whole).unwrap();
        for gid in 0..cff.glyph_count() {
            assert_eq!(whole.charstring(gid), cff.charstring(gid));
        }
        eprintln!(
            "{style}: {} glyphs, CFF {} bytes, subset of 7 {} bytes; subroutines {before:?}, kept {after:?}",
            cff.glyph_count(),
            table.len(),
            subset.len()
        );
    }
}
