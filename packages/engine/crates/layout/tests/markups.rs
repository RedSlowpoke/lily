//! Text and markup (MS3 step 8) natively, in a VM without the library:
//! Pango's font descriptions, text stencils over a text pack made by hand,
//! with the numbers `pango.rs` documents (sizes, advances, kerning and ink
//! in Pango units, rounded as Pango 1.58.2 and HarfBuzz 14.4.0 round them),
//! ligatures, features, a character the face has not and the spaces
//! HarfBuzz falls back for; runs by script and by face (MS4 step 19);
//! `ly:perform-text-replacements`; the text interface's callbacks and
//! their errors, and `print` and `grob-interpret-markup` on a grob made in
//! a hand-made layout (MS3 step 19; skipped without LilyPond 2.26.0's
//! music types); the font functions of a Pango font. And the shaping check
//! (MS4 step 19): every string each reference of `$LILY_CORPUS/ref/` draws
//! in LilyPond's own text fonts (D106), in its pages and markups
//! (`tools/compare/stencils.ly`), made again from its description and text
//! over W1's text packs at its score's staff size: the capture's
//! `utf-8-string`, number for number, but the strings of fonts no pack
//! holds (T20's and T26's, D227). Those skip without LilyPond
//! 2.26.0 or the captures, and are then unverified; the markups themselves,
//! interpreted with their chains by the library, are
//! `tools/compare/test/stencils.test.mjs`'.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use lily_engine_layout::font;
use lily_engine_layout::pango::{Description, Style, face_file};
use lily_engine_scheme::{Value, Vm};

fn engine() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `fetch.mjs`' corpus directory: `LILY_CORPUS`, or `~/.cache/lily-engine`.
fn corpus() -> PathBuf {
    std::env::var_os("LILY_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap()).join(".cache/lily-engine")
        })
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

/// `source`'s value, written; the test fails if it throws.
fn ok(vm: &mut Vm, source: &str) -> String {
    let (key, text) = run(vm, source);
    assert_eq!(key, "", "{source}: {text}");
    text
}

/// A VM with the music crate's and layout's functions, `(lily)` current;
/// `paper`, an output definition of staff size 20 as
/// `layout-set-absolute-staff-size-in-module` sets one, scaled as
/// `paper-book.cc` scales a book's; `(chain props …)`, a chain of `props`
/// as `Font_interface::text_font_alist_chain` ends one, with LilyPond's own
/// text families (D106); and the warnings in `seen`.
fn vm() -> Vm {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    lily_engine_layout::install(&mut vm).unwrap();
    ok(
        &mut vm,
        "(set-current-module (resolve-module '(lily)))
         (define staff-height (* 20 (ly:pt 1)))
         (define paper (ly:make-output-def))
         (ly:output-def-set-variable! paper 'staff-height staff-height)
         (ly:output-def-set-variable! paper 'output-scale (/ staff-height 4))
         (ly:output-def-set-variable! paper 'text-font-size 11.0)
         (define fonts '((fonts (serif . \"LilyPond Serif\") (sans . \"LilyPond Sans Serif\")
                                (typewriter . \"LilyPond Monospace\") (music . \"emmentaler\"))))
         (define (chain . props) (list props '((font-encoding . latin1)) fonts))
         (define seen '())
         (set! %warned (lambda (texts value) (set! seen (append seen texts)) value))
         #t",
    );
    vm
}

// --- a text pack by hand ---------------------------------------------------------

/// `tools/packs/text.mjs`' format, written.
struct Writer(Vec<u8>);

impl Writer {
    fn varint(&mut self, mut v: u64) {
        while v >= 0x80 {
            self.0.push((v as u8 & 0x7f) | 0x80);
            v >>= 7;
        }
        self.0.push(v as u8);
    }

    fn zigzag(&mut self, v: i64) {
        self.varint(if v < 0 {
            (-2 * v - 1) as u64
        } else {
            2 * v as u64
        });
    }

    fn string(&mut self, s: &str) {
        self.varint(s.len() as u64);
        self.0.extend_from_slice(s.as_bytes());
    }
}

/// A face `name` of six glyphs: `.notdef` (500 wide), `A` (250, ink from
/// (-10, -20) to (90, 280)), `B` (750, ink to (600, 700)), `space` (300),
/// `f_f` (900, ink (0, -5) to (880, 705)) and `f` (450, ink (10, 0) to
/// (440, 705)); the characters space, `A`, `B` and `f`, and `more`'s
/// (after them, ascending); `A` before `B` kerned by -50; `liga` making
/// `f f` `f_f`. The metrics' pack (`LILT` 4, D262): its outlines are
/// [`outlines_pack`]'s.
fn text_pack(name: &str, more: &[(char, i64)]) -> Vec<u8> {
    let glyphs = GLYPHS;
    let mut w = Writer(b"LILT\x04".to_vec());
    for s in [name, "C059", "Roman"] {
        w.string(s);
    }
    w.varint(400);
    w.0.push(0);
    w.varint(1000);
    w.varint(glyphs.len() as u64);
    for (_, advance, _) in glyphs {
        w.varint(advance);
    }
    for (name, _, _) in glyphs {
        w.string(name);
    }
    let mut cmap = vec![(' ', 3), ('A', 1), ('B', 2), ('f', 5)];
    cmap.extend_from_slice(more);
    w.varint(cmap.len() as u64);
    let (mut c0, mut g0) = (None::<u32>, 0i64);
    for (c, g) in cmap {
        w.varint(c0.map_or(c as u64, |p| (c as u32 - p - 1) as u64));
        w.zigzag(g - g0);
        (c0, g0) = (Some(c as u32), g);
    }
    // One first glyph, A, with one pair, B.
    w.varint(1);
    w.varint(1);
    w.varint(1);
    w.varint(2);
    w.zigzag(-50);
    // One lookup, a ligature substitution of one ligature, f f into f_f,
    // which liga names.
    w.varint(1);
    w.0.push(4);
    w.varint(1);
    w.varint(2);
    w.varint(5);
    w.varint(5);
    w.varint(4);
    w.varint(1);
    w.string("liga");
    w.varint(1);
    w.varint(0);
    // Each ink box: its corner, its width and height.
    for (_, _, [x0, y0, x1, y1]) in glyphs {
        w.zigzag(x0);
        w.zigzag(y0);
        w.varint((x1 - x0) as u64);
        w.varint((y1 - y0) as u64);
    }
    w.0
}

/// [`text_pack`]'s six glyphs: name, advance, ink box.
const GLYPHS: [(&str, u64, [i64; 4]); 6] = [
    (".notdef", 500, [0; 4]),
    ("A", 250, [-10, -20, 90, 280]),
    ("B", 750, [0, 0, 600, 700]),
    ("space", 300, [0; 4]),
    ("f_f", 900, [0, -5, 880, 705]),
    ("f", 450, [10, 0, 440, 705]),
];

/// The outlines of [`text_pack`]'s face `name` (`LILO` 1, D262).
fn outlines_pack(name: &str) -> Vec<u8> {
    let mut w = Writer(b"LILO\x01".to_vec());
    w.string(name);
    w.varint(GLYPHS.len() as u64);
    // Each ink box as an outline: a move to one corner, a line to the other.
    for (_, _, [x0, y0, x1, y1]) in GLYPHS {
        let mut o = Writer(Vec::new());
        if [x0, y0, x1, y1] != [0; 4] {
            o.0.push(0);
            o.zigzag(x0);
            o.zigzag(y0);
            o.0.push(1);
            o.zigzag(x1 - x0);
            o.zigzag(y1 - y0);
        }
        w.varint(o.0.len() as u64);
        w.0.extend(o.0);
    }
    w.0
}

/// Gives `vm` [`text_pack`]'s face `name` and its outlines.
fn put_face(vm: &mut Vm, name: &str, more: &[(char, i64)]) {
    font::put_pack(vm, &text_pack(name, more)).unwrap();
    font::put_pack(vm, &outlines_pack(name)).unwrap();
}

/// [`vm`] given [`text_pack`] as `C059-Roman`, with `k`, the Pango font's
/// `scale_` at `paper`'s output scale.
fn texting() -> Vm {
    let mut vm = vm();
    put_face(&mut vm, "C059-Roman", &[]);
    ok(
        &mut vm,
        "(define k (/ 72 (* 1024 1200 (ly:output-def-lookup paper 'output-scale)))) #t",
    );
    vm
}

#[test]
fn descriptions_read_and_write_as_pangos() {
    let mut d = Description::new("LilyPond Serif");
    d.size = Some(3959);
    assert_eq!(d.to_string(), "LilyPond Serif 3.866");
    d.weight = 700;
    d.style = Style::Italic;
    assert_eq!(d.to_string(), "LilyPond Serif Bold Italic 3.866");
    assert_eq!(d.to_filename(), "lilypond_serif_bold_italic_3.866");
    // The fewest decimals that read back to the size, as Pango 1.58.2
    // writes each from 0 to 40,000 Pango units.
    for (size, text) in [
        (21, "0.0205"),
        (64, "0.0625"),
        (1024, "1"),
        (1979, "1.933"),
        (4444, "4.34"),
        (40000, "39.0625"),
    ] {
        let mut d = Description::new("F");
        d.size = Some(size);
        assert_eq!(d.to_string(), format!("F {text}"));
    }
    let mut all = Description::new("F");
    all.weight = 600;
    all.style = Style::Oblique;
    all.stretch = 2;
    all.small_caps = true;
    assert_eq!(all.to_string(), "F Semi-Bold Oblique Condensed Small-Caps");
    assert_eq!(Description::new("").to_string(), "Normal");

    let read = Description::from_string("Foo, Bar Bold Italic 12");
    assert_eq!(
        (read.family.as_str(), read.weight, read.style, read.size),
        ("Foo, Bar", 700, Style::Italic, Some(12288))
    );
    assert_eq!(
        Description::from_string("LilyPond Serif Bold 3.866").to_string(),
        "LilyPond Serif Bold 3.866"
    );
    assert_eq!(Description::from_string("Serif").size, None);
}

#[test]
fn the_backend_is_svg_and_its_text_families_are_c059s() {
    // `main.cc:662–663`'s backend when a run names none, and
    // `lily.scm:562–566`'s option with it, as LilyPond's `--svg` runs have.
    let mut vm = vm();
    assert_eq!(
        ok(
            &mut vm,
            "(list (ly:get-option 'backend) (ly:get-option 'music-strings-to-paths))"
        ),
        "(svg #t)"
    );
    // `paper-defaults-init.ly:170–181`'s families under svg are the faces
    // of LilyPond's own, which D106's references name.
    for (svg, own) in [
        ("serif", "LilyPond Serif"),
        ("sans", "LilyPond Sans Serif"),
        ("monospace", "LilyPond Monospace"),
    ] {
        for (weight, style) in [
            (400, Style::Normal),
            (700, Style::Normal),
            (400, Style::Italic),
            (700, Style::Italic),
        ] {
            let face = |family: &str| {
                let mut d = Description::new(family);
                (d.weight, d.style) = (weight, style);
                face_file(&d, false)
            };
            assert_eq!(face(svg), face(own), "{svg} {weight} {style:?}");
        }
    }
    assert_eq!(face_file(&Description::new("serif"), false), "C059-Roman");
}

#[test]
fn a_string_is_a_glyph_string_with_pangos_numbers() {
    let mut vm = texting();
    // 11 points are 3,959 Pango units, the font loaded at 65,983 (the
    // pixels at 1,200 dpi, in Pango units), which it describes as 3,958.
    // A: HarfBuzz's 16,496 and kerning's -3,299 make 13,197, 13 pixels;
    // its ink from floor(280 × 65.983) = 18,475 down to ceil(-20 × 65.983)
    // = -1,319. B: 49,487, 48 pixels, its ink up to 46,188.
    assert_eq!(
        ok(
            &mut vm,
            "(equal? (%stencil->dump (ly:text-interface::interpret-markup paper (chain) \"AB\"))
               `((0.0 . ,(* 62464 k)) (,(* -1319 k) . ,(* 46188 k))
                 (utf-8-string \"LilyPond Serif 3.866\" \"AB\"
                   (translate-stencil (0.0 . 0.0)
                     (glyph-string (%font #f \"unknown\" 1.0) \"C059-Roman\" 3.865234375 #f
                       ((,(* 13312 k) (,(* -1319 k) . ,(* 18475 k)) 0.0 0.0 1 \"A\")
                        (,(* 49152 k) (0.0 . ,(* 46188 k)) 0.0 0.0 2 \"B\"))
                       \"<datadir>/fonts/otf/C059-Roman.otf\" 0 \"AB\" ((1 . 1) (1 . 1)))))))"
        ),
        "#t"
    );
    // f f is f_f, one glyph of two bytes; white space is a space.
    assert_eq!(
        ok(
            &mut vm,
            "(let ((e (caddr (%stencil->dump (ly:text-interface::interpret-markup paper (chain) \"ff\tA\")))))
               (list (caddr e) (map (lambda (g) (list-ref g 5)) (list-ref (caddr (cadddr e)) 5))
                     (list-ref (caddr (cadddr e)) 9)))"
        ),
        "(\"ff A\" (\"f_f\" \"space\" \"A\") ((2 . 1) (1 . 1) (1 . 1)))"
    );
    // Neither ligatures nor kerning with the features turned off.
    assert_eq!(
        ok(
            &mut vm,
            "(let ((e (caddr (%stencil->dump (ly:text-interface::interpret-markup
                                              paper (chain '(font-features \"-liga\" \"kern=0\")) \"ffAB\")))))
               (list (map (lambda (g) (list-ref g 5)) (list-ref (caddr (cadddr e)) 5))
                     (equal? (car (car (list-ref (caddr (cadddr e)) 5))) (* 29696 k))
                     (equal? (car (caddr (list-ref (caddr (cadddr e)) 5))) (* 16384 k))))"
        ),
        "((\"f\" \"f\" \"A\" \"B\") #t #t)"
    );
    // A character the face has not: LilyPond's warning, no glyph, no
    // kerning across it and no clusters.
    assert_eq!(
        ok(
            &mut vm,
            "(let ((e (caddr (%stencil->dump (ly:text-interface::interpret-markup paper (chain) \"AéB\")))))
               (list (map (lambda (g) (list-ref g 5)) (list-ref (caddr (cadddr e)) 5))
                     (equal? (car (car (list-ref (caddr (cadddr e)) 5))) (* 16384 k))
                     (list-ref (caddr (cadddr e)) 9) seen))"
        ),
        "((\"A\" \"B\") #t #f (\"no glyph for character 'é' (U+00E9 LATIN SMALL LETTER E WITH ACUTE) in font `<datadir>/fonts/otf/C059-Roman.otf'\"))"
    );
    // Spaces the face has not, as HarfBuzz falls back for them: the face's
    // space at an em's fifth for U+2009, (65,983 + 2) / 5 = 13,197, 13
    // pixels; at half its own for U+202F, 19,794 / 2 = 9,897, 10 pixels; at
    // its own for U+2008, the face having no full stop or comma; no warning.
    assert_eq!(
        ok(
            &mut vm,
            "(set! seen '())
             (let ((e (caddr (%stencil->dump (ly:text-interface::interpret-markup paper (chain) \"A\u{2009}B\u{202f}A\u{2008}\")))))
               (list (map (lambda (g) (list (inexact->exact (round (/ (car g) k))) (list-ref g 5))) (list-ref (caddr (cadddr e)) 5))
                     (list-ref (caddr (cadddr e)) 9) seen))"
        ),
        "(((16384 \"A\") (13312 \"space\") (49152 \"B\") (10240 \"space\") (16384 \"A\") (19456 \"space\")) ((1 . 1) (3 . 1) (1 . 1) (3 . 1) (1 . 1) (3 . 1)) ())"
    );
    // An empty string makes no run: the empty stencil.
    assert_eq!(
        ok(
            &mut vm,
            "(%stencil->dump (ly:text-interface::interpret-markup paper (chain) \"\"))"
        ),
        "((+inf.0 . -inf.0) (+inf.0 . -inf.0) ())"
    );
}

/// The runs of a string, as `(text font-file x (glyph …))` of each glyph
/// string, `x` its offset in Pango units.
const RUNS: &str = "(define (runs markup)
  (let loop ((x (cadddr (caddr (%stencil->dump markup)))) (out '()))
    (cond ((eq? (car x) 'translate-stencil)
           (cons (list (list-ref (caddr x) 8) (list-ref (caddr x) 6)
                       (inexact->exact (round (/ (car (cadr x)) k)))
                       (map (lambda (g) (list-ref g 5)) (list-ref (caddr x) 5)))
                 out))
          ((eq? (car x) 'combine-stencil)
           (let each ((xs (cdr x)) (out out))
             (if (null? xs) out (each (cdr xs) (loop (car xs) out)))))
          (else out))))";

#[test]
fn runs_split_by_script_and_by_face_each_after_the_one_before() {
    let mut vm = texting();
    // A second face, Nimbus Sans's, with é (B's glyph) and Ж (A's).
    put_face(&mut vm, "NimbusSans-Regular", &[('é', 2), ('Ж', 1)]);
    ok(&mut vm, &format!("{RUNS} #t"));
    let both = "(chain '(font-name . \"LilyPond Serif, Nimbus Sans\"))";
    // é, which C059's face has not, from Nimbus Sans, a run of its own
    // after A's 16,496, B after its 49,487; no kerning across the runs.
    assert_eq!(
        ok(
            &mut vm,
            &format!(
                "(set! seen '()) (list (runs (ly:text-interface::interpret-markup paper {both} \"AéB\")) seen)"
            )
        ),
        "(((\"A\" \"<datadir>/fonts/otf/C059-Roman.otf\" 0 (\"A\")) \
          (\"é\" \"<datadir>/fonts/otf/NimbusSans-Regular.otf\" 16384 (\"B\")) \
          (\"B\" \"<datadir>/fonts/otf/C059-Roman.otf\" 65536 (\"B\"))) ())"
            .replace("          ", "")
    );
    // Spaces after Nimbus Sans's é go with the A after them, C059's being
    // the earlier face; a space after A stays with it.
    assert_eq!(
        ok(
            &mut vm,
            &format!(
                "(map car (runs (ly:text-interface::interpret-markup paper {both} \"é A é\")))"
            )
        ),
        "(\"é\" \" A \" \"é\")"
    );
    // A script's run: Cyrillic Ж, in Nimbus Sans; the parenthesis before
    // it in the run it opens in, Latin's, and the one closing it in Latin.
    assert_eq!(
        ok(
            &mut vm,
            &format!(
                "(map car (runs (ly:text-interface::interpret-markup paper {both} \"A (Ж) B\")))"
            )
        ),
        "(\"A (\" \"Ж\" \") B\")"
    );
    // No face after C059's for a font of one family: Ж, in a run of its
    // own, no glyph and the warning, without a name beyond names.rs'.
    assert_eq!(
        ok(
            &mut vm,
            "(set! seen '()) (list (map car (runs (ly:text-interface::interpret-markup paper (chain) \"AЖ\"))) seen)"
        ),
        "((\"A\") (\"no glyph for character 'Ж' (U+0416) in font `<datadir>/fonts/otf/C059-Roman.otf'\"))"
    );
}

#[test]
fn a_text_font_is_a_pango_font_of_its_face() {
    let mut vm = texting();
    put_face(&mut vm, "C059-Bold", &[]);
    assert_eq!(
        ok(
            &mut vm,
            "(let ((f (ly:paper-get-font paper (chain '(font-encoding . latin1) '(font-series . bold)))))
               (list f (ly:pango-font? f) (ly:font-metric? f) (ly:font-file-name f) (ly:font-name f)
                     (ly:font-magnification f) (eq? f (ly:paper-get-font paper (chain '(font-series . bold))))
                     (ly:pango-font-physical-fonts f)))"
        ),
        "(#<Pango_font (#f . 1.0)> #t #t #f \"unknown\" 1.0 #t ())"
    );
    assert_eq!(
        ok(
            &mut vm,
            "(let* ((props (chain '(font-series . bold) '(font-size . -2)))
                    (s (ly:text-interface::interpret-markup paper props \"A\"))
                    (f (ly:paper-get-font paper props)))
               (list (cadr (caddr (%stencil->dump s))) (ly:pango-font-physical-fonts f)
                     (ly:font-glyph-name-to-index f \"B\") (ly:font-glyph-name-to-index f \".notdef\")
                     (ly:stencil-expr (ly:font-get-glyph f \"A\")) (ly:font-index-to-charcode f 2)
                     (length (ly:output-def-lookup paper 'pango-fonts))))"
        ),
        "(\"LilyPond Serif Bold 3.068\" ((\"C059-Bold\" \"<datadir>/fonts/otf/C059-Bold.otf\" 0)) 2 -1 \
         (named-glyph #<Pango_font (#f . 1.0)> \"A\") 2 4)"
    );
    // LilyPond's warnings of the font properties, and a face no pack gave.
    put_face(&mut vm, "C059-Italic", &[]);
    assert_eq!(
        ok(
            &mut vm,
            "(ly:pango-font? (ly:paper-get-font paper
               (chain '(font-shape . slanted) '(font-series . fat) '(font-variant . caps))))"
        ),
        "#t"
    );
    assert_eq!(
        ok(&mut vm, "seen"),
        "(\"unknown font-variant value caps; allowed values are normal and small-caps\" \
         \"unknown font-series value fat; allowed values are thin, ultralight (or extralight), light, \
semilight (or demilight), book, normal (or regular), medium, semibold (or demibold), bold, ultrabold \
(or extrabold), heavy (or black), and ultraheavy (or ultrablack or extrablack)\")"
    );
    let (key, text) = run(
        &mut vm,
        "(ly:paper-get-font paper (chain '(font-family . sans)))",
    );
    assert_eq!(
        (key.as_str(), text.as_str()),
        ("misc-error", "cannot find font 'NimbusSans-Regular'")
    );
    // What is no text font gives the empty stencil.
    assert_eq!(
        ok(
            &mut vm,
            "(%stencil->dump (%text-stencil (ly:make-stencil '()) paper '() \"x\"))"
        ),
        "((+inf.0 . -inf.0) (+inf.0 . -inf.0) ())"
    );
}

#[test]
fn text_replacements_take_the_longest_string_at_each_place() {
    let mut vm = vm();
    ok(
        &mut vm,
        "(define (make-concat-markup l) (cons 'concat l))
         (define props '(((replacement-alist (\"ab\" . \"X\") (\"a\" . \"Y\") (\"abc\" . \"Z\") (\"ab\" . \"no\")
                                             (\"é\" . \"E\") (\"\" . \"none\") (x . \"none\")))))
         #t",
    );
    for (input, output) in [
        ("abcab", "(concat \"\" \"Z\" \"\" \"X\" \"\")"),
        // At the first a the largest string at or before "axb" is "abc",
        // which is not there, so "a" is not replaced (LilyPond's map).
        ("xaxbé", "(concat \"xaxb\" \"E\" \"\")"),
        ("xa", "(concat \"x\" \"Y\" \"\")"),
        ("no", "(concat \"no\")"),
        ("", "\"\""),
    ] {
        assert_eq!(
            ok(
                &mut vm,
                &format!("(ly:perform-text-replacements paper props \"{input}\")")
            ),
            output,
            "{input}"
        );
    }
    // The largest string at or before the text, not a shorter one that
    // matches: "a" is not tried when "ab" is the largest.
    assert_eq!(
        ok(
            &mut vm,
            "(ly:perform-text-replacements paper '(((replacement-alist (\"a\" . \"1\") (\"ab\" . \"2\")))) \"ac\")"
        ),
        "(concat \"ac\")"
    );
    assert_eq!(
        ok(
            &mut vm,
            "(ly:perform-text-replacements paper '(((replacement-alist))) \"abc\")"
        ),
        "\"abc\""
    );
}

#[test]
fn the_text_interfaces_callbacks_check_their_arguments() {
    let mut vm = texting();
    for (source, text) in [
        (
            "(ly:text-interface::interpret-markup 'paper '() \"x\")",
            "In procedure ly:text-interface::interpret-markup: Wrong type argument in position 1 (expecting Output_def): paper",
        ),
        (
            "(ly:text-interface::print 'grob)",
            "In procedure ly:text-interface::print: Wrong type argument in position 1 (expecting Grob): grob",
        ),
        (
            "(ly:text-interface::grob-interpret-markup 'grob \"x\")",
            "In procedure ly:text-interface::grob-interpret-markup: Wrong type argument in position 1 (expecting Grob): grob",
        ),
        (
            "(ly:text-interface::interpret-markup paper (chain '(font-features . \"liga\")) \"x\")",
            "In procedure interpret_string: Expecting a list for font-features value",
        ),
        (
            "(ly:text-interface::interpret-markup paper (chain '(font-features liga)) \"x\")",
            "In procedure interpret_string: Found non-string in font-features list",
        ),
    ] {
        let (key, got) = run(&mut vm, source);
        assert_ne!(key, "", "{source}");
        assert_eq!(got, text, "{source}");
    }
    // A string transformer, applied before the string is measured.
    assert_eq!(
        ok(
            &mut vm,
            "(caddr (caddr (%stencil->dump (ly:text-interface::interpret-markup paper
               (chain (cons 'string-transformers (list (lambda (layout props s) (string-append s \"B\")))))
               \"A\"))))"
        ),
        "\"AB\""
    );
    // Not a markup: LilyPond's programming error and the empty stencil.
    assert_eq!(
        ok(
            &mut vm,
            "(list (ly:stencil-expr (ly:text-interface::interpret-markup paper '() 7))
                   (vector-ref (car (car %diagnostics)) 2))"
        ),
        "(() \"Trying to interpret a non-markup object: 7\")"
    );
    for source in [
        "(ly:text-interface::interpret-markup paper '())",
        "(ly:text-interface::grob-interpret-markup 'grob)",
        "(ly:text-interface::print)",
        "(ly:perform-text-replacements paper '())",
        "(ly:pango-font?)",
        "(ly:pango-font-physical-fonts 1 2)",
    ] {
        let (key, text) = run(&mut vm, source);
        assert_eq!(key, "wrong-number-of-args", "{source}: {text}");
    }
}

/// `scm/lily/` as `build.rs` finds it.
fn lily_scm() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("LILY_SCM") {
        return Some(PathBuf::from(dir));
    }
    let path = Command::new("sh")
        .args(["-c", "command -v lilypond"])
        .output()
        .ok()?;
    let binary = std::fs::canonicalize(String::from_utf8(path.stdout).ok()?.trim()).ok()?;
    Some(binary.parent()?.join("../share/lilypond/2.26.0/scm/lily"))
}

/// The grobs `Text_maker` made in `paper`, translated over two notes:
/// `(text 0)` and `(text 1)`, TextScripts whose definition prints with
/// `ly:text-interface::print`, a `text` of `"AB"` and LilyPond's text
/// families; `None` without LilyPond 2.26.0's music types.
fn texted() -> Option<(Vm, lily_engine_music::Translated)> {
    use lily_engine_music::{TYPES, make_context_def, run_translator_with};
    if TYPES.is_empty() {
        eprintln!("skipped: no music types without LilyPond 2.26.0");
        return None;
    }
    let mut vm = texting();
    ok(
        &mut vm,
        "(define-public INF-MOMENT (ly:make-moment +inf.0))
         (define-public ZERO-MOMENT (ly:make-moment 0 1))
         #t",
    );
    let callbacks = lily_scm()?.join("define-music-callbacks.scm");
    let text = std::fs::read_to_string(&callbacks).ok()?;
    vm.eval_text(&text, callbacks.to_str()).unwrap();
    lily_engine_music::define_music_types(&mut vm).unwrap();
    ok(
        &mut vm,
        "(define (note step)
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 step 0) 'duration (ly:make-duration 2)))
         (define-public (ly:make-event-class class) (list class))
         (define-public made '())
         (define-public (text n) (list-ref made n))
         (ly:register-translator
           (lambda (context)
             (let ((heard #f))
               `((listeners (note-event . ,(lambda (t ev) (set! heard ev))))
                 (process-music
                  . ,(lambda (t)
                       (when heard
                         (set! made (append made (list (ly:engraver-make-grob t 'TextScript heard))))
                         (set! heard #f)))))))
           'Text_maker)
         #t",
    );
    let defs = [
        "`((context-name Global) (accepts Score) (default-child Score)
           (grob-descriptions
            ((TextScript (stencil . ,ly:text-interface::print) (text . \"AB\")
                         (fonts (serif . \"LilyPond Serif\") (sans . \"LilyPond Sans Serif\")
                                (typewriter . \"LilyPond Monospace\") (music . \"emmentaler\"))
                         (meta (name . TextScript) (interfaces text-interface grob-interface))))))",
        "'((context-name Score) (translator-type Score_engraver) (accepts Voice) (default-child Voice))",
        "'((context-name Voice) (translator-type Engraver_group) (consists Text_maker))",
    ];
    let paper = ok_value(&mut vm, "paper");
    let scope = ok_value(&mut vm, "(ly:output-def-scope paper)");
    for text in defs {
        let mods = ok_value(&mut vm, text);
        let mut items = Vec::new();
        let mut p = mods;
        while let (Some(m), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
            items.push(m);
            p = rest;
        }
        let (def, unknown) = make_context_def(vm.heap_mut(), None, &items, Value::FALSE);
        assert_eq!(unknown, 0, "{text}");
        let name = vm.heap().car(vm.heap().cdr(items[0]).unwrap()).unwrap();
        let name = vm.heap().name(name).unwrap().to_string();
        vm.module_define(scope, &name, def);
    }
    let music = ok_value(
        &mut vm,
        "(make-music 'SequentialMusic 'elements (list (note 0) (note 1)))",
    );
    vm.heap_mut().root(music);
    let done = run_translator_with(&mut vm, music, paper, None, &[])
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    Some((vm, done))
}

/// `source`'s value; the test fails if it throws.
fn ok_value(vm: &mut Vm, source: &str) -> Value {
    vm.eval_text(source, None)
        .unwrap_or_else(|e| panic!("{source}: {}", e.text(vm.heap())))
}

#[test]
fn the_text_interface_prints_a_grob_in_its_layout_with_its_text_font_chain() {
    let Some((mut vm, _done)) = texted() else {
        return;
    };
    // `text_font_alist_chain`: the grob's own properties, its definition,
    // then `((font-encoding . latin1))`; and the grob's layout. The two
    // properties set have types, as define-grob-properties.scm gives them
    // (`text` a string: no `markup?` without the library).
    ok(
        &mut vm,
        "(set-object-property! 'font-size 'backend-type? number?)
         (set-object-property! 'text 'backend-type? string?)
         (ly:grob-set-property! (text 0) 'font-size 3)
         (define (by-hand props markup)
           (%stencil->dump
             (ly:text-interface::interpret-markup
               paper
               (list props (cadr (ly:grob-alist-chain (text 0) '())) '((font-encoding . latin1)))
               markup)))
         #t",
    );
    assert_eq!(ok(&mut vm, "(eq? (ly:grob-layout (text 0)) paper)"), "#t");
    // `print`: the grob's `text`, through its `stencil`.
    let printed = ok(
        &mut vm,
        "(%stencil->dump (ly:grob-property (text 0) 'stencil))",
    );
    assert_eq!(printed, ok(&mut vm, "(by-hand '((font-size . 3)) \"AB\")"));
    assert!(printed.contains("\"LilyPond Serif "), "{printed}");
    // `grob-interpret-markup`: any markup in the same chain.
    assert_eq!(
        ok(
            &mut vm,
            "(%stencil->dump (ly:text-interface::grob-interpret-markup (text 0) \"BA\"))"
        ),
        ok(&mut vm, "(by-hand '((font-size . 3)) \"BA\")")
    );
    // The other grob, without the font size; killed by its `text`
    // callback, read first: the empty stencil.
    let other = ok(
        &mut vm,
        "(%stencil->dump (ly:text-interface::print (text 1)))",
    );
    assert_ne!(other, printed);
    assert_eq!(other, ok(&mut vm, "(by-hand '() \"AB\")"));
    // The layout and the chain themselves, as a string transformer given
    // to the first grob hears them.
    assert_eq!(
        ok(
            &mut vm,
            "(set-object-property! 'string-transformers 'backend-type? list?)
             (define heard #f)
             (ly:grob-set-property! (text 0) 'string-transformers
               (list (lambda (layout props s) (set! heard (cons layout props)) s)))
             (ly:text-interface::print (text 0))
             (list (eq? (car heard) paper)
                   (equal? (cdr heard) (ly:grob-alist-chain (text 0) '((font-encoding . latin1))))
                   (list-ref heard 3))"
        ),
        "(#t #t ((font-encoding . latin1)))"
    );
    assert_eq!(
        ok(
            &mut vm,
            "(ly:grob-set-property! (text 1) 'text (lambda (grob) (ly:grob-suicide! grob) \"AB\"))
             (%stencil->dump (ly:text-interface::print (text 1)))"
        ),
        "((+inf.0 . -inf.0) (+inf.0 . -inf.0) ())"
    );
}

// --- the shaping check: every reference's strings ---------------------------------------

/// The text packs `tools/packs/text.mjs` builds, once; `None` without
/// LilyPond 2.26.0.
fn packs() -> Option<&'static Path> {
    static BUILT: OnceLock<Option<PathBuf>> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            let version = Command::new("lilypond").arg("--version").output().ok()?;
            if !String::from_utf8_lossy(&version.stdout).contains("GNU LilyPond 2.26.0") {
                return None;
            }
            let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("layout-markups");
            std::fs::remove_dir_all(&dir).ok();
            std::fs::create_dir_all(&dir).unwrap();
            let run = Command::new("node")
                .arg(engine().join("tools/packs/text.mjs"))
                .arg("--out")
                .arg(&dir)
                .output()
                .expect("node runs tools/packs/text.mjs");
            assert!(
                run.status.success(),
                "tools/packs/text.mjs failed: {}",
                String::from_utf8_lossy(&run.stderr)
            );
            Some(dir)
        })
        .as_deref()
}

/// Each `utf-8-string` of a record made again: the text in the description
/// it names, at its size (a `text-font-size` that gives it, `font-size` 0),
/// in a paper of the score's staff size, its `glyph-string`s' files by
/// their names only; `(#t)` if it is the capture's, `(face)` if it is but
/// for the face it names (each glyph string's PostScript name, file and
/// glyph indices), else the capture's and the engine's.
const AGAIN: &str = r#"
(define (probe-strings x acc)
  (cond ((and (pair? x) (eq? (car x) 'utf-8-string) (pair? (cdr x)) (string? (cadr x))) (cons x acc))
        ((pair? x) (probe-strings (cdr x) (probe-strings (car x) acc)))
        (else acc)))
(define (probe-base s)
  (let ((i (string-rindex s #\/))) (if i (substring s (1+ i)) s)))
(define (probe-norm x)
  (cond ((and (pair? x) (eq? (car x) 'glyph-string) (= (length x) 10))
         (append (list-head x 6) (list (probe-base (list-ref x 6))) (list-tail x 7)))
        ((pair? x) (cons (probe-norm (car x)) (probe-norm (cdr x))))
        (else x)))
(define (probe-faceless x)
  (cond ((and (pair? x) (eq? (car x) 'glyph-string) (= (length x) 10))
         (append (list (car x) (cadr x) (list-ref x 3) (list-ref x 4))
                 (list (map (lambda (g) (append (list-head g 4) (list-tail g 5))) (list-ref x 5)))
                 (list-tail x 7)))
        ((pair? x) (cons (probe-faceless (car x)) (probe-faceless (cdr x))))
        (else x)))
(define (probe-size description)
  (let* ((words (string-split description #\space))
         (size (string->number (car (last-pair words)))))
    (inexact->exact (floor (+ (* size 1024) 0.5)))))
(define (probe-paper staff)
  (let ((layout (ly:output-def-clone paper))
        (height (* staff (ly:pt 1))))
    (ly:output-def-set-variable! layout 'staff-height height)
    (ly:output-def-set-variable! layout 'output-scale (/ height 4))
    layout))
(define (probe-again node staff)
  (let* ((description (cadr node))
         (layout (probe-paper staff)))
    (ly:output-def-set-variable! layout 'text-font-size
                                 (/ (probe-size description) 1024 (ly:pt 1)))
    (let* ((s (ly:text-interface::interpret-markup layout (list (list (cons 'font-name description))) (caddr node)))
           (got (probe-norm (caddr (%stencil->dump s))))
           (want (probe-norm node)))
      (cond ((equal? got want) '(#t))
            ((equal? (probe-faceless got) (probe-faceless want)) '(face))
            (else (list want got))))))
"#;

/// The references not at staff size 20, as their files set it: T21's,
/// T23's, T25's and T29's `set-global-staff-size`, T28's in
/// `cary-layout.ily`, T30's `layout-set-staff-size` in its `\paper`, and
/// Y45's (`episema.ly`, MS8 step 9's part 2) `set-global-staff-size`.
const STAFF: [(&str, u32); 10] = [
    ("T21", 24),
    ("T23", 15),
    ("T25", 14),
    ("T28", 14),
    ("T29", 10),
    ("T30", 14),
    ("Y45", 26),
    ("Y51", 21),
    ("Y54", 21),
    ("Y55", 21),
];

/// The references whose text LilyPond drew in fonts no pack holds, which the
/// capturing machine's fontconfig chose (D227), and what of their strings
/// is the engine's: T20's lyrics, in Times New Roman and Hiragino Mincho,
/// compared apart and recorded (D224). T26's "New Century Schoolbook", which
/// fontconfig gives LilyPond's own TeX Gyre Schola, was LilyPond's but for
/// the face in C059 until its pack (E23, D271): now LilyPond's. Y51, Y54
/// and Y55 (MS8 step 9's part 3) include typography-demo.ly, which asks
/// for T20's families: all their text is in the machine's Times New Roman
/// and its fallbacks, recorded as T20's (D227).
const APART: [(&str, &str); 4] = [
    ("T20", "recorded"),
    ("Y51", "recorded"),
    ("Y54", "recorded"),
    ("Y55", "recorded"),
];

#[test]
fn every_references_strings_in_lilyponds_text_fonts_are_lilyponds() {
    let Some(packs) = packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the strings are not made again");
        return;
    };
    let refs = corpus().join("ref");
    let ids: Vec<String> = std::fs::read_dir(&refs)
        .map(|dir| {
            dir.filter_map(|e| e.ok()?.file_name().into_string().ok())
                .filter(|id| refs.join(id).join("markups.scm").exists())
                .collect()
        })
        .unwrap_or_default();
    let thirty = (1..=30).map(|n| format!("T{n:02}"));
    if !thirty.clone().all(|id| ids.contains(&id)) {
        eprintln!(
            "skipped: no markups.scm of the 30 in {}: run npm run capture; the strings are not made again",
            refs.display()
        );
        return;
    }
    let packs: Vec<Vec<u8>> = std::fs::read_dir(packs)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "pack"))
        .map(|p| std::fs::read(p).unwrap())
        .collect();
    let mut ids = ids;
    ids.sort();
    let (mut strings, mut styles) = (0, std::collections::BTreeMap::new());
    let mut apart = std::collections::BTreeMap::new();
    for id in &ids {
        // A VM of its own: a Pango font is made at the output scale it is
        // first asked for.
        let mut vm = vm();
        for pack in &packs {
            font::put_pack(&mut vm, pack).unwrap();
        }
        ok(&mut vm, &format!("{AGAIN} #t"));
        let staff = STAFF.iter().find(|(s, _)| s == id).map_or(20, |&(_, n)| n);
        let judged = APART.iter().find(|(s, _)| s == id).map(|&(_, how)| how);
        for file in ["pages.scm", "markups.scm"] {
            let text = std::fs::read_to_string(refs.join(id).join(file)).unwrap();
            for line in text.lines().filter(|l| l.contains("(utf-8-string ")) {
                let source = format!(
                    "(map (lambda (n) (list (cadr n) (probe-again n {staff})))
                          (probe-strings '{line} '()))"
                );
                let value = vm
                    .eval_text(&source, None)
                    .unwrap_or_else(|e| panic!("{id} {file}: {}", e.text(vm.heap())));
                let heap = vm.heap();
                let mut at = value;
                while let (Some(r), Some(rest)) = (heap.car(at), heap.cdr(at)) {
                    let d = heap.str(heap.car(r).unwrap()).unwrap().as_str().to_string();
                    let mut result = String::new();
                    heap.write(heap.car(heap.cdr(r).unwrap()).unwrap(), &mut result);
                    let kind = match result.as_str() {
                        "(#t)" => "LilyPond's",
                        "(face)" => "LilyPond's but for the face",
                        _ => "other",
                    };
                    match (judged, kind) {
                        (_, "LilyPond's") | (Some("face"), "LilyPond's but for the face") => {}
                        (Some("recorded"), _) => {}
                        _ => panic!("{id} {file}: a string not LilyPond's: {result}"),
                    }
                    if judged.is_some() {
                        *apart.entry(format!("{id} {kind}")).or_insert(0) += 1;
                    }
                    let face = d.rsplit_once(' ').map_or(d.as_str(), |(face, _)| face);
                    *styles.entry(face.to_string()).or_insert(0) += 1;
                    strings += 1;
                    at = rest;
                }
            }
        }
    }
    eprintln!(
        "{strings} strings of {} references made again: {styles:?}; apart: {apart:?}",
        ids.len()
    );
    // T26's time signatures in TeX Gyre Schola's pack (E23) are LilyPond's like the rest.
    assert!(!apart.keys().any(|k| k.starts_with("T26")), "{apart:?}");
    assert_eq!(apart.get("T20 other"), Some(&77));
    // typography-demo.ly's strings in Times New Roman (D227).
    assert_eq!(apart.get("Y51 other"), Some(&122));
    assert_eq!(apart.get("Y54 other"), Some(&116));
    assert_eq!(apart.get("Y55 other"), Some(&116));
    assert!(strings >= 1_504, "{strings} strings");
}
