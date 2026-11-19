//! O2 part 1 (MS3 step 4): LilyPond's font metrics over W1's packs against
//! LilyPond 2.26.0.
//!
//! - `tools/bench/probe-fonts.scm`'s calls give what LilyPond gives for them
//!   (`tools/compare/font-calls.ly`), in a VM with only the music crate's
//!   and layout's functions, given the packs of emmentaler-20,
//!   emmentaler-13 and emmentaler-brace that `tools/packs/build.mjs` builds
//!   from the installed LilyPond's OTFs: every glyph's index, character,
//!   `LILC` entry and `ly:font-get-glyph` stencil, the fonts' names, design
//!   sizes and glyph lists, and the functions' argument errors. Skipped
//!   without LilyPond 2.26.0, and then unverified;
//!   `tools/bench/test/fonts.test.mjs` runs the probe from source, from an
//!   image and in wasm.
//! - Without LilyPond: a pack made by hand read as LilyPond reads an OTF,
//!   the packs a host gives and the fonts it asks for, and the placeholders.
//! - Font selection (MS3 step 5), without LilyPond: `ly:paper-get-font` of
//!   hand-made packs named as Emmentaler's, in an output definition made
//!   by hand: the optical size and the magnification, the scaled font each
//!   definition keeps, its glyphs scaled, `ly:reset-all-fonts`, the
//!   warnings and the text fonts' placeholder. `tools/bench/test/
//!   fonts.test.mjs` checks what the 10 scores select against LilyPond.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use lily_engine_layout::font;
use lily_engine_scheme::Vm;

/// The packs the probe loads, in the order a host gives them.
const PACKS: [&str; 6] = [
    "emmentaler-20",
    "emmentaler-13",
    "emmentaler-brace-0",
    "emmentaler-brace-1",
    "emmentaler-brace-2",
    "emmentaler-brace-3",
];

/// A VM with the music crate's and layout's functions, `(lily)` current.
fn vm() -> Vm {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    lily_engine_layout::install(&mut vm).unwrap();
    vm.eval_text("(set-current-module (resolve-module '(lily)))", None)
        .unwrap();
    vm
}

/// What `source` gives: an empty key and the value as `write` writes it, or
/// the error's key and its text as Guile prints it.
fn run(vm: &mut Vm, source: &str) -> (String, String) {
    match vm.eval_text(source, None) {
        Ok(value) => {
            let mut out = String::new();
            vm.heap().write(value, &mut out);
            (String::new(), out)
        }
        Err(e) => {
            let key = vm.heap().name(e.key).unwrap_or("?").to_string();
            (key, e.text(vm.heap()))
        }
    }
}

fn engine() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A probe's output, one record per call.
fn records(output: &str) -> Vec<String> {
    let mut records: Vec<String> = output.split("\u{1e}\n").map(str::to_string).collect();
    assert_eq!(
        records.pop().as_deref(),
        Some(""),
        "the output ends a record"
    );
    records
}

/// The packs `build.mjs` built and what `font-calls.ly` gives, made once
/// for the tests; `None` without LilyPond 2.26.0.
fn built() -> Option<&'static (PathBuf, Vec<String>)> {
    static BUILT: OnceLock<Option<(PathBuf, Vec<String>)>> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            let version = Command::new("lilypond").arg("--version").output().ok()?;
            if !String::from_utf8_lossy(&version.stdout).contains("GNU LilyPond 2.26.0") {
                return None;
            }
            let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("layout-fonts");
            std::fs::remove_dir_all(&dir).ok();
            std::fs::create_dir_all(&dir).unwrap();
            let packs = Command::new("node")
                .arg(engine().join("tools/packs/build.mjs"))
                .arg("--out")
                .arg(&dir)
                .output()
                .expect("node runs tools/packs/build.mjs");
            assert!(
                packs.status.success(),
                "tools/packs/build.mjs failed: {}",
                String::from_utf8_lossy(&packs.stderr)
            );
            let capture =
                std::fs::canonicalize(engine().join("tools/compare/font-calls.ly")).unwrap();
            let out = Command::new("lilypond")
                .arg("-s")
                .arg(capture)
                .current_dir(&dir)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            Some((dir, records(&String::from_utf8(out.stdout).unwrap())))
        })
        .as_ref()
}

#[test]
fn the_probe_gives_what_lilypond_gives() {
    let Some((dir, reference)) = built() else {
        eprintln!(
            "skipped: LilyPond 2.26.0 is not installed: the font metrics are unverified against it"
        );
        return;
    };
    let mut vm = vm();
    for pack in PACKS {
        let bytes = std::fs::read(dir.join(format!("{pack}.pack"))).unwrap();
        font::put_pack(&mut vm, &bytes).unwrap_or_else(|e| panic!("{pack}: {}", e.text(vm.heap())));
    }
    let probe = std::fs::read_to_string(engine().join("tools/bench/probe-fonts.scm")).unwrap();
    let result = vm.eval_text(&probe, None);
    let output = vm.take_output();
    if let Err(e) = result {
        panic!(
            "{}\n{}",
            &output[output.len().saturating_sub(400)..],
            e.text(vm.heap())
        );
    }
    let got = records(&output);
    // The written calls, then each font and its glyphs: 664, 664 and 577.
    assert!(
        reference.len() > 3 + 664 + 664 + 577,
        "LilyPond ran every call"
    );
    assert_eq!(got.len(), reference.len(), "every call");
    let differ: Vec<_> = got
        .iter()
        .zip(reference)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, (a, b))| format!("call {}:\n  engine:   {a}\n  LilyPond: {b}", i + 1))
        .take(10)
        .collect();
    assert!(
        differ.is_empty(),
        "the calls that differ:\n{}",
        differ.join("\n")
    );
}

/// Two glyphs by hand of a font `test`, PostScript name `Test`, of design
/// size 20: `a`, the character 97, with LILC's bbox (-0.000 -3.125 7.500
/// 0.000) and ledger-shortening-range (-4.000 . 5.000); `b`, no character's,
/// without LILC, a move to (0, -4) and a line to (1, -3) (crates/fonts'
/// own).
const PACK: &[u8] = b"LILP\x03\x04test\x04Test\x00\xe8\x07\x03\x01\x0bdesign_size\x01\x90\x03\x02\x01a\x01b\xf4\x03\x00\xc3\x01\x00\x09\x01\xeb\x30\x98\x75\x00\xc1\x3e\x90\x4e\x00\x00\x06\x00\x00\x07\x01\x02\x02\x00\x00\x00";

/// `PACK` as `#vu8(…)`.
fn bytevector(pack: &[u8]) -> String {
    let bytes: Vec<String> = pack.iter().map(u8::to_string).collect();
    format!("#vu8({})", bytes.join(" "))
}

#[test]
fn a_pack_is_read_as_lilypond_reads_an_otf() {
    let mut vm = vm();
    font::put_pack(&mut vm, PACK).unwrap();
    let cases = [
        (
            "(let ((f (ly:system-font-load \"test\"))) (list f (ly:font-name f) (ly:font-file-name f) (ly:font-magnification f) (ly:otf-glyph-count f) (ly:otf-glyph-list f)))",
            "",
            "(#<Open_type_font (\"test\" . 1.0)> \"Test\" \"test\" 1.0 1 (\"a\" \"b\"))",
        ),
        (
            "(= (ly:font-design-size (ly:system-font-load \"test\")) (* 20 (ly:pt 1)))",
            "",
            "#t",
        ),
        (
            "(let ((f (ly:system-font-load \"test\"))) (map (lambda (n) (list (ly:font-glyph-name-to-index f n) (ly:otf-font-glyph-info f n))) '(\"a\" \"b\" \"c\")))",
            "",
            "((-1 ((bbox -0.0 -3.125 7.5 0.0) (ledger-shortening-range -4.0 . 5.0))) (1 ()) (-1 ()))",
        ),
        (
            "(let ((f (ly:system-font-load \"test\"))) (map (lambda (i) (ly:font-index-to-charcode f i)) '(-1 0 1 2)))",
            "",
            "(0 97 0 0)",
        ),
        // b's box is the control box of its outline, 20 points per 1000
        // units; glyph 0 is FreeType's missing glyph.
        (
            "(let* ((f (ly:system-font-load \"test\")) (b (ly:font-get-glyph f \"b\")) (s (/ (* 20 (ly:pt 1)) 1000))) (list (ly:stencil-expr b) (equal? (ly:stencil-extent b 0) (cons 0.0 s)) (equal? (ly:stencil-extent b 1) (cons (* -4 s) (* -3 s))) (ly:stencil-expr (ly:font-get-glyph f \"a\"))))",
            "",
            "((named-glyph #<Open_type_font (\"test\" . 1.0)> \"b\") #t #t ())",
        ),
        (
            "(ly:system-font-load \"emmentaler-20\")",
            "misc-error",
            "cannot find font 'emmentaler-20'",
        ),
    ];
    for (source, key, value) in cases {
        let (k, v) = run(&mut vm, source);
        assert_eq!((k.as_str(), v.as_str()), (key, value), "{source}");
    }
}

#[test]
fn the_packs_a_host_gives_make_its_fonts() {
    let mut vm = vm();
    let put = |vm: &mut Vm, pack: &[u8]| {
        run(
            vm,
            &format!("((@@ (lily) %put-font-pack) {})", bytevector(pack)),
        )
    };
    // One glyph `a`, then the part after it, `b`, of the font `f` (crates/fonts').
    let one = b"LILP\x03\x01f\x01F\x00\xe8\x07\x00\x00\x01\x01a\x00\x05\x00\x00\x00\x00\x00";
    let next = b"LILP\x03\x01f\x01F\x01\xe8\x07\x00\x00\x01\x01b\x00\x03\x00\x00\x00\x00\x00";
    assert_eq!(
        put(&mut vm, next).0,
        "misc-error",
        "a part that continues no font"
    );
    put(&mut vm, one);
    assert_eq!(
        run(&mut vm, "(ly:otf-glyph-list (ly:system-font-load \"f\"))").1,
        "(\"a\")"
    );
    let f = run(&mut vm, "(define f (ly:system-font-load \"f\"))");
    assert_eq!(f.0, "");
    put(&mut vm, next);
    // The font a part continues is the font that was loaded.
    assert_eq!(
        run(&mut vm, "(list (eq? f (ly:system-font-load \"f\")) (ly:otf-glyph-list f) (ly:otf-glyph-count f))").1,
        "(#t (\"a\" \"b\") 2)"
    );
    assert_eq!(
        put(&mut vm, next),
        (
            "misc-error".to_string(),
            "In procedure %put-font-pack: a font pack that does not continue its font".to_string()
        )
    );
    assert_eq!(
        put(&mut vm, b"OTTO"),
        (
            "misc-error".to_string(),
            "In procedure %put-font-pack: not a font pack of W1's".to_string()
        )
    );
    assert_eq!(
        run(&mut vm, "((@@ (lily) %put-font-pack) \"LILP\")").1,
        "In procedure %put-font-pack: Wrong type argument in position 1 (expecting bytevector): \"LILP\""
    );
}

/// The five that read what the packs do not hold check their arguments as
/// LilyPond does (the probe), then throw by name.
#[test]
fn the_placeholders_throw_by_name() {
    let mut vm = vm();
    font::put_pack(&mut vm, PACK).unwrap();
    for (call, subr, reads) in [
        (
            "(ly:otf-font-table-data (ly:system-font-load \"test\") \"LILC\")",
            "ly:otf-font-table-data",
            "the font's tables",
        ),
        (
            "(ly:get-font-format \"emmentaler-20.otf\")",
            "ly:get-font-format",
            "a font file's format",
        ),
        (
            "(ly:has-glyph-names? \"emmentaler-20.otf\" 0)",
            "ly:has-glyph-names?",
            "a font file's glyph names",
        ),
        (
            "(ly:get-cff-offset \"emmentaler-20.otf\" -1)",
            "ly:get-cff-offset",
            "a font file's CFF offset",
        ),
        (
            "(ly:extract-subfont-from-collection \"a.otc\" 1 \"b.otf\")",
            "ly:extract-subfont-from-collection",
            "a font collection's file",
        ),
    ] {
        assert_eq!(
            run(&mut vm, call),
            (
                "misc-error".to_string(),
                format!("In procedure {subr}: reads {reads}, which W1's packs do not hold")
            ),
            "{call}"
        );
    }
}

/// [`PACK`] under the font name `name`.
fn named(name: &str) -> Vec<u8> {
    let mut pack = PACK[..5].to_vec();
    pack.push(name.len() as u8);
    pack.extend_from_slice(name.as_bytes());
    pack.extend_from_slice(&PACK[10..]);
    pack
}

/// A VM given [`PACK`] as `emmentaler-20`, `emmentaler-13`,
/// `emmentaler-brace` and `LilyPond Serif-20`, with
/// `feta-design-size-mapping` as `lily-library.scm` defines it, `paper`,
/// an output definition of staff size `size` as
/// `layout-set-absolute-staff-size-in-module` sets one, and `%warned`
/// keeping the warnings in `seen`.
fn selecting(size: f64) -> Vm {
    let mut vm = vm();
    for name in [
        "emmentaler-20",
        "emmentaler-13",
        "emmentaler-brace",
        "LilyPond Serif-20",
    ] {
        font::put_pack(&mut vm, &named(name)).unwrap();
    }
    let setup = format!(
        "(define-public feta-design-size-mapping
           '((11 . 11.22) (13 . 12.60) (14 . 14.14) (16 . 15.87) (18 . 17.82) (20 . 20)
             (23 . 22.45) (26 . 25.20)))
         (define staff-height (* {size} (ly:pt 1)))
         (define paper (ly:make-output-def))
         (ly:output-def-set-variable! paper 'staff-height staff-height)
         (ly:output-def-set-variable! paper 'output-scale (/ staff-height 4))
         (ly:output-def-set-variable! paper 'text-font-size (* 11 (/ staff-height (* 20 (ly:pt 1)))))
         (define fonts '((fonts (music . \"emmentaler\") (serif . \"LilyPond Serif\"))))
         (define (chain . props) (list props fonts))
         (define seen '())
         (set! %warned (lambda (texts value) (set! seen (append seen texts)) value))"
    );
    let (key, text) = run(&mut vm, &setup);
    assert_eq!(key, "", "{text}");
    vm
}

#[test]
fn a_music_font_is_selected_as_lilypond_selects_it() {
    let mut vm = selecting(20.0);
    // At staff size 20 Emmentaler-20 at its size: its magnification is one
    // over output-scale.
    for (source, value) in [
        (
            "(let ((f (ly:paper-get-font paper (chain '(font-encoding . fetaMusic)))))
               (list (ly:font-file-name f) (ly:font-name f) (= (ly:font-design-size f) (* 20 (ly:pt 1)))
                     (= (ly:font-magnification f) (/ (/ (/ staff-height (ly:pt 1)) 20) (/ staff-height 4)))
                     (ly:font-metric? f) (ly:otf-font? f) (ly:otf-glyph-count f) (ly:otf-glyph-list f)
                     (ly:font-glyph-name-to-index f \"a\") (ly:font-index-to-charcode f 0)
                     (ly:otf-font-glyph-info f \"a\")))",
            "(\"emmentaler-20\" \"Test\" #t #t #t #t 1 (\"a\" \"b\") -1 97 ((bbox -0.0 -3.125 7.5 0.0) (ledger-shortening-range -4.0 . 5.0)))",
        ),
        // The same font each time, scaled from the font ly:system-font-load
        // gives; a glyph's box scaled by the magnification.
        (
            "(let* ((f (ly:paper-get-font paper (chain '(font-encoding . fetaMusic))))
                    (otf (ly:system-font-load \"emmentaler-20\"))
                    (m (ly:font-magnification f))
                    (b (ly:font-get-glyph f \"b\")) (o (ly:font-get-glyph otf \"b\")))
               (list (eq? f (ly:paper-get-font paper (chain '(font-encoding . fetaMusic))))
                     (eq? f (ly:paper-get-font (ly:output-def-clone paper) (chain '(font-encoding . fetaMusic))))
                     (equal? (ly:stencil-expr b) (list 'named-glyph f \"b\"))
                     (equal? (ly:stencil-extent b 0) (cons (* m (car (ly:stencil-extent o 0))) (* m (cdr (ly:stencil-extent o 0)))))
                     (equal? (ly:stencil-extent b 1) (cons (* m (car (ly:stencil-extent o 1))) (* m (cdr (ly:stencil-extent o 1)))))
                     (ly:stencil-expr (ly:font-get-glyph f \"c\"))))",
            "(#t #t #t #t #t ())",
        ),
        // font-size -4 asks for 12.599 points: Emmentaler-13, whose design
        // size is 12.60, scaled by what is left over; a font-family of serif
        // is music's.
        (
            "(let ((f (ly:paper-get-font paper (chain '(font-encoding . fetaMusic) '(font-size . -4) '(font-family . serif)))))
               (list f (= (ly:font-magnification f) (/ (/ (* 20 (expt 2 (/ -4 6.0))) 12.60) (/ staff-height 4)))))",
            "",
        ),
        (
            "(ly:font-file-name (ly:paper-get-font paper (chain '(font-encoding . fetaBraces) '(font-size . 3))))",
            "\"emmentaler-brace\"",
        ),
        (
            "(let ((f (ly:paper-get-font paper (chain '(font-encoding . fetaMusic)))))
               (ly:reset-all-fonts)
               (list (eq? f (ly:paper-get-font paper (chain '(font-encoding . fetaMusic))))
                     (ly:font-file-name (ly:paper-get-font paper (chain '(font-encoding . fetaMusic))))))",
            "(#f \"emmentaler-20\")",
        ),
        ("seen", "()"),
    ] {
        let (key, text) = run(&mut vm, source);
        if value.is_empty() {
            assert_eq!(key, "", "{source}: {text}");
            assert!(
                text.starts_with("(#<Modified_font_metric (\"emmentaler-13\" . ") && text.ends_with(")> #t)"),
                "{source}: {text}"
            );
        } else {
            assert_eq!((key.as_str(), text.as_str()), ("", value), "{source}");
        }
    }
}

#[test]
fn a_font_family_the_fonts_do_not_name_is_warned_of() {
    let mut vm = selecting(20.0);
    let (key, text) = run(
        &mut vm,
        "(list (ly:font-file-name (ly:paper-get-font paper (chain '(font-encoding . fetaMusic) '(font-family . roman))))
               (ly:font-file-name (ly:paper-get-font paper (list '((font-encoding . fetaMusic)) '((fonts (music . 7))))))
               seen)",
    );
    assert_eq!(key, "", "{text}");
    assert_eq!(
        text,
        "(\"LilyPond Serif-20\" \"LilyPond Serif-20\" (\"no entry for font family roman in fonts alist\" \"expected string for value in fonts alist, found: 7\"))"
    );
}

#[test]
fn a_text_font_is_a_pango_font_of_its_pack() {
    let mut vm = selecting(20.0);
    for (source, key, text) in [
        (
            "(ly:paper-get-font paper (chain '(font-encoding . latin1)))",
            "misc-error",
            "cannot find font 'C059-Roman'",
        ),
        (
            "(ly:paper-get-font paper (chain '(font-encoding . fetaText)))",
            "",
            "#<Pango_font (#f . 1.0)>",
        ),
        (
            "(ly:font-name (ly:paper-get-font paper (chain '(font-encoding . fetaText))))",
            "",
            "\"unknown\"",
        ),
        (
            "(ly:paper-get-font paper (chain '(font-encoding . fetaMusic) '(font-name . \"Serif Bold\")))",
            "misc-error",
            "cannot find font 'C059-Bold'",
        ),
        (
            "(ly:paper-get-font paper (chain '(font-encoding . wrong) '(font-shape . slanted)))",
            "misc-error",
            "cannot find font 'C059-Italic'",
        ),
        (
            "(ly:paper-get-font 'paper '())",
            "wrong-type-arg",
            "In procedure ly:paper-get-font: Wrong type argument in position 1 (expecting Output_def): paper",
        ),
        (
            "(ly:paper-get-font (ly:make-output-def) (chain '(font-encoding . fetaMusic)))",
            "wrong-type-arg",
            "Wrong type (expecting real number): #<undefined>",
        ),
    ] {
        let (got_key, got) = run(&mut vm, source);
        assert_eq!((got_key.as_str(), got.as_str()), (key, text), "{source}");
    }
}
