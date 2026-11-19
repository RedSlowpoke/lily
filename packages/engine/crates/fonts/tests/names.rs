//! The Unicode names LilyPond's "no glyph for character" warning gives (MS4
//! step 19): `unicode::formal_name` against Guile's `char->formal-name`,
//! which LilyPond 2.26.0 calls, for every character of `names.rs`' blocks,
//! the Hangul syllables and the CJK ideographs; and `names.rs` as
//! tools/packs/names.scm writes it. Skipped without a Guile 3.0 on the PATH.
use std::fmt::Write;
use std::path::Path;
use std::process::Command;

use lily_engine_fonts::unicode::formal_name;

/// The characters checked: names.rs' blocks, the Hangul syllables, the
/// CJK compatibility ideographs and those past them, and the first and
/// last of the CJK unified ideographs' blocks, which Guile does not name.
const RANGES: [(u32, u32); 12] = [
    (0x20, 0x7E),
    (0xA0, 0xFF),
    (0x591, 0x5F4),
    (0x3000, 0x30FF),
    (0x3400, 0x3401),
    (0x4DBF, 0x4DBF),
    (0x4E00, 0x4E01),
    (0x9FFF, 0x9FFF),
    (0xAC00, 0xD7A3),
    (0xF900, 0xFAFF),
    (0x20000, 0x20001),
    (0x2F800, 0x2FA1F),
];

/// Guile's names of every character of [`RANGES`], `#f` for none, one a line.
fn guile() -> Option<Vec<String>> {
    let mut source = String::from("(use-modules (ice-9 unicode))\n");
    for (a, b) in RANGES {
        writeln!(
            source,
            "(do ((c {a} (1+ c))) ((> c {b})) (display (or (char->formal-name (integer->char c)) \"#f\")) (newline))"
        )
        .unwrap();
    }
    let run = Command::new("guile")
        .args(["--no-auto-compile", "-c", &source])
        .output()
        .ok()?;
    run.status.success().then(|| {
        String::from_utf8(run.stdout)
            .unwrap()
            .lines()
            .map(String::from)
            .collect()
    })
}

#[test]
fn names_are_guiles_char_formal_name() {
    let Some(names) = guile() else {
        eprintln!("skipped: no guile on the PATH: the names are not checked");
        return;
    };
    let ours: Vec<String> = RANGES
        .iter()
        .flat_map(|&(a, b)| a..=b)
        .filter_map(char::from_u32)
        .map(|c| formal_name(c).unwrap_or_else(|| String::from("#f")))
        .collect();
    assert_eq!(ours.len(), names.len());
    let differ: Vec<String> = RANGES
        .iter()
        .flat_map(|&(a, b)| a..=b)
        .zip(ours.iter().zip(&names))
        .filter(|(_, (a, b))| a != b)
        .map(|(c, (a, b))| format!("U+{c:04X}: {a} for {b}"))
        .collect();
    assert_eq!(differ, Vec::<String>::new());
}

#[test]
fn names_rs_is_what_names_scm_writes() {
    let engine = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok(run) = Command::new("guile")
        .arg("--no-auto-compile")
        .arg(engine.join("tools/packs/names.scm"))
        .output()
    else {
        eprintln!("skipped: no guile on the PATH: names.rs is not written again");
        return;
    };
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let file = std::fs::read_to_string(engine.join("crates/fonts/src/names.rs")).unwrap();
    // rustfmt leaves the generated file as it is.
    assert_eq!(String::from_utf8(run.stdout).unwrap(), file);
}
