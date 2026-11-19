//! The SVG writer (MS3 step 7, W3) natively.
//!
//! - The file form by hand, without LilyPond: a page of each primitive the
//!   writer knows, in a font of a pack made by hand, written as 06 §6.2
//!   says, character for character.
//! - The 11 pages of the 10 scores of MS3's gate, from step 6's references
//!   (`$LILY_CORPUS/ref/<id>/pages.scm`, written by `npm run capture`), read
//!   into layout's stencils and fonts given the packs `tools/packs/build.mjs`
//!   builds, and written: no warnings, each glyph defined once a page, and
//!   every link's file, line, char and column and its `translate` LilyPond's
//!   page's, in order. Skipped without LilyPond 2.26.0 or the references,
//!   and then unverified; `tools/compare/test/svg.test.mjs` compares the
//!   pages' pixels and sizes.
//! - The link an engine grob's print stencil writes (`output-svg.scm`'s
//!   `grob-cause`): its event cause's origin, with each value of
//!   `point-and-click`, and none for a grob caused by a grob. Skipped
//!   without LilyPond 2.26.0's music types, and then unverified.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use std::rc::Rc;

use lily_engine_layout::grob::get_print_stencil;
use lily_engine_layout::{Stencil, font};
use lily_engine_music::{Input, OutputDef, Translated, make_context_def, run_translator_with};
use lily_engine_scheme::{Value, Vm};
use lily_engine_svg::{Markup, Paper, write_page, write_preview};

/// The 10 scores of MS3's gate (README §5).
const TEN: [&str; 10] = [
    "T01", "T03", "T05", "T10", "T13", "T14", "T16", "T17", "T18", "T19",
];

/// The packs the pages draw with: Emmentaler-20, -13 (T10's text) and the
/// braces (T13).
const PACKS: [&str; 6] = [
    "emmentaler-13",
    "emmentaler-20",
    "emmentaler-brace-0",
    "emmentaler-brace-1",
    "emmentaler-brace-2",
    "emmentaler-brace-3",
];

/// crates/fonts' pack by hand of a font `test` (PostScript `Test`, design
/// size 20): `a` without an outline, `b` a move to (0, -4) and a line to
/// (1, -3).
const PACK: &[u8] = b"LILP\x03\x04test\x04Test\x00\xe8\x07\x03\x01\x0bdesign_size\x01\x90\x03\x02\x01a\x01b\xf4\x03\x00\xc3\x01\x00\x09\x01\xeb\x30\x98\x75\x00\xc1\x3e\x90\x4e\x00\x00\x06\x00\x00\x07\x01\x02\x02\x00\x00\x00";

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

/// A VM with the music crate's and layout's functions, `(lily)` current,
/// given `packs`.
fn vm(packs: &[Vec<u8>]) -> Vm {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    lily_engine_layout::install(&mut vm).unwrap();
    vm.eval_text("(set-current-module (resolve-module '(lily)))", None)
        .unwrap();
    for pack in packs {
        font::put_pack(&mut vm, pack).unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    }
    vm
}

/// The stencil of a `(page …)` record and its file name.
fn read(vm: &mut Vm, line: &str) -> (String, Stencil) {
    let source =
        format!("(let ((r '{line})) (cons (cadr r) (apply %dump->stencil (list-tail r 3))))");
    let value = vm
        .eval_text(&source, None)
        .unwrap_or_else(|e| panic!("{}: {}", &line[..line.len().min(80)], e.text(vm.heap())));
    let heap = vm.heap();
    let name = heap
        .str(heap.car(value).unwrap())
        .unwrap()
        .as_str()
        .to_string();
    (
        name,
        *heap.foreign::<Stencil>(heap.cdr(value).unwrap()).unwrap(),
    )
}

const BY_HAND: &str = r#"(page "a.svg" 1 (0.0 . 119.5) (-169.0 . 0.0) (translate-stencil (8.5 . 0.0) (combine-stencil (grob-cause "textedit:///root/dir/sub/a.ly:3:4:5" (combine-stencil (named-glyph (%font "test" "Test" 0.5) "b") (translate-stencil (1.0 . 0.0) (draw-line 0.1 0 0 1.0 0)))) (translate-stencil (1.0 . -2.0) (named-glyph (%font "test" "Test" 0.5) "b")) (named-glyph (%font "test" "Test" 0.5) "a") (grob-cause #f (round-filled-box 0.1 0.2 0.3 0.4 0.08)) (dashed-line 0.1 0.5 1.0 3.0 0 0) (utf-8-string "LilyPond Serif Bold 3.866" "a<b&c" ()) (color (1 0 0 1.0) (circle 1 0.3 #f)) (glyph-string (%font #f "unknown" 1.0) "Test" 7.029296875 #f ((1.0 (0.0 . 1.0) 0.0 0.0 1 "b") (1.0 (0.0 . 1.0) 0.0 0.5 1 "b")) "test.otf" 0 "bb" ((1 . 1) (1 . 1))) (url-link "https://lilypond.org" (0.0 . 17.0) (-0.45 . 1.62)) (rotate-stencil (90 (0 . 0)) (polygon (0 0 1 0 1 1) 0.1 #t)) (scale-stencil (2 1) (path 0.1 (moveto 0 0 lineto 1 1 closepath) butt round #f)) (embedded-svg "<x/>") (grob-cause "textedit:///elsewhere/b.ly:1:0:1" (embedded-svg "<x/>")))))"#;

const BY_HAND_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" version="1.2" width="210mm" height="296.98mm" viewBox="0 0 119.5 169">
<style type="text/css">
<![CDATA[
tspan { white-space: pre; }
a { color: inherit }
]]>
</style>
<g fill="currentColor" stroke-linejoin="round" stroke-linecap="round">
<a xlink:href="textedit:///sub/a.ly:3:4:5">
<g transform="translate(8.5, 0)">
<defs><path id="lily-g-test-b-0.0035" transform="scale(.0035, -.0035)" d="M0-4l1 1z"/></defs>
<use xlink:href="#lily-g-test-b-0.0035"/>
</g>
<line stroke-width=".1" stroke="currentColor" x1="9.5" y1="0" x2="10.5" y2="0"/>
</a>
<use xlink:href="#lily-g-test-b-0.0035" x="9.5" y="2"/>
<rect x="8.4" y="-.4" width=".3" height=".7" ry=".04"/>
<line stroke-width=".1" stroke="currentColor" x1="8.5" y1="0" x2="11.5" y2="0" stroke-dasharray=".5,1"/>
<g transform="translate(8.5, 0)">
<text font-family="LilyPond Serif" font-weight="bold" font-size="2.2" text-anchor="start">
<tspan>a&lt;b&amp;c</tspan>
</text>
</g>
<g color="rgba(100%, 0%, 0%, 100%)">
<g transform="translate(8.5, 0)">
<circle fill="none" stroke="currentColor" stroke-width=".3" r="1"/>
</g>
</g>
<defs><path id="lily-g-test-b-0.0040" transform="scale(.004, -.004)" d="M0-4l1 1z"/></defs>
<use xlink:href="#lily-g-test-b-0.0040" x="8.5" y="0"/>
<use xlink:href="#lily-g-test-b-0.0040" x="9.5" y=".5"/>
<g transform="translate(8.5, 0)">
<a xlink:href="https://lilypond.org">
<rect x="0" y="-.45" width="17" height="2.07" fill="none" stroke="none" stroke-width="0"/>
</a>
</g>
<g transform="rotate(-90, 8.5, 0)">
<g transform="translate(8.5, 0)">
<polygon stroke-width=".1" stroke="currentColor" points="0 0 1 0 1 -1"/>
</g>
</g>
<g transform="scale(2, 1)">
<g transform="translate(4.25, 0)">
<path stroke-width=".1" stroke-linecap="butt" stroke="currentColor" fill="none" d="M0 0L1 -1z"/>
</g>
</g>
<a xlink:href="textedit:///elsewhere/b.ly:1:0:1">
<g transform="translate(8.5, 0)">
</g>
</a>
</g>
</svg>
"##;

#[test]
fn the_file_form_by_hand() {
    let mut vm = vm(&[PACK.to_vec()]);
    let (name, stencil) = read(&mut vm, BY_HAND);
    assert_eq!(name, "a.svg");
    let roots = ["/root/dir", "/root"];
    let paper = Paper {
        output_scale: Paper::output_scale_of(20.0),
        tree_roots: &roots,
    };
    let page = write_page(&mut vm, &stencil, &paper).unwrap();
    assert_eq!(page.warnings, Vec::<String>::new());
    assert!(page.svg == BY_HAND_SVG, "the page differs:\n{}", page.svg);
}

/// A preview's entry is taken from the last compile only for the same
/// numbers (OPEN B32): two widths 2·10⁻¹¹ apart, either side of a fourth
/// decimal, are written each as it prints, as a writer of its own would.
#[test]
fn a_preview_entry_is_taken_only_for_its_own_numbers() {
    let mut vm = vm(&[]);
    let roots = ["/root"];
    let paper = Paper {
        output_scale: Paper::output_scale_of(20.0),
        tree_roots: &roots,
    };
    let page = |w: &str| {
        format!(
            "(page \"a.svg\" 1 (0.0 . 119.5) (-169.0 . 0.0) (round-filled-box 0.1 {w} 0.3 0.4 0.08))"
        )
    };
    let write = |vm: &mut Vm, markup: &mut Markup, w: &str| {
        let (_, stencil) = read(vm, &page(w));
        markup.begin();
        let svg = write_preview(vm, &stencil, &paper, markup).unwrap().svg;
        markup.end(0);
        svg
    };
    let mut kept = Markup::default();
    let before = write(&mut vm, &mut kept, "1.70524999999");
    assert!(before.contains("width=\"1.8052\""), "{before}");
    let after = write(&mut vm, &mut kept, "1.70525000001");
    let fresh = write(&mut vm, &mut Markup::default(), "1.70525000001");
    assert!(fresh.contains("width=\"1.8053\""), "{fresh}");
    assert_eq!(after, fresh);
}

/// `scm/lily/` of the installed LilyPond, as `build.rs` finds it.
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

fn eval(vm: &mut Vm, text: &str) -> Value {
    vm.eval_text(text, None)
        .unwrap_or_else(|e| panic!("{text}: {}", e.text(vm.heap())))
}

/// A VM translated over a note at `/s/a b.ly`'s line 3, character 4 and
/// column 6, whose Scheme engraver makes a TextScript of it, `(g 0)`, and
/// one of that grob, `(g 1)`, each with a box for its stencil; and the
/// installed `ly:in-event-class?`. `None` without LilyPond 2.26.0's music
/// types.
fn translated() -> Option<(Vm, Translated)> {
    if lily_engine_music::TYPES.is_empty() {
        eprintln!(
            "skipped: no music types without LilyPond 2.26.0: an engine grob's link is unverified"
        );
        return None;
    }
    let mut vm = vm(&[]);
    // `grob-cause`'s `any`, as the image's `(lily)` has it.
    eval(&mut vm, "(use-modules (srfi srfi-1))");
    eval(
        &mut vm,
        "(define-public INF-MOMENT (ly:make-moment +inf.0))
         (define-public ZERO-MOMENT (ly:make-moment 0 1))",
    );
    let lily = lily_scm().unwrap();
    let callbacks = lily.join("define-music-callbacks.scm");
    let text = std::fs::read_to_string(&callbacks).unwrap();
    vm.eval_text(&text, callbacks.to_str()).unwrap();
    lily_engine_music::define_music_types(&mut vm).unwrap();
    let text = std::fs::read_to_string(lily.join("define-event-classes.scm")).unwrap();
    let start = text.find("(define-public (ly:in-event-class?").unwrap();
    let mut heap = lily_engine_scheme::Heap::new();
    let form = lily_engine_scheme::read(&mut heap, &text, start, None).unwrap();
    eval(&mut vm, &text[form.start..form.end]);
    let loc = Input::new(0, 10, 12, Rc::from("/s/a b.ly"), [3, 4, 6]);
    let loc = vm.heap_mut().wrap(loc);
    vm.define("loc", loc);
    eval(
        &mut vm,
        "(define-public (ly:make-event-class class) (list class))
         (define-public made '())
         (define (g n) (list-ref made n))
         (ly:register-translator
           (lambda (context)
             (let ((heard #f))
               `((listeners (note-event . ,(lambda (t ev) (set! heard ev))))
                 (process-music
                  . ,(lambda (t)
                       (when heard
                         (let* ((text (ly:engraver-make-grob t 'TextScript heard))
                                (other (ly:engraver-make-grob t 'TextScript text)))
                           (set! made (list text other)))
                         (set! heard #f)))))))
           'Text_maker)",
    );
    let scope = vm.make_module(&["lily"]).unwrap();
    let layout = vm.heap_mut().wrap(OutputDef::new(scope));
    vm.heap_mut().root(layout);
    for text in [
        "((context-name Global) (accepts Score) (default-child Score)
          (grob-descriptions
           ((TextScript (meta (name . TextScript) (interfaces grob-interface))))))",
        "((context-name Score) (translator-type Score_engraver) (accepts Voice)
          (default-child Voice))",
        "((context-name Voice) (translator-type Engraver_group) (consists Text_maker))",
    ] {
        let mods = eval(&mut vm, &format!("'{text}"));
        let mut items = Vec::new();
        let mut p = mods;
        while let (Some(m), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
            items.push(m);
            p = rest;
        }
        let (def, _) = make_context_def(vm.heap_mut(), None, &items, Value::FALSE);
        let name = vm.heap().car(vm.heap().cdr(items[0]).unwrap()).unwrap();
        let name = vm.heap().name(name).unwrap().to_string();
        vm.module_define(scope, &name, def);
    }
    let music = eval(
        &mut vm,
        "(make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration 2)
                     'origin loc)",
    );
    vm.heap_mut().root(music);
    let done = run_translator_with(&mut vm, music, layout, None, &[])
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    eval(
        &mut vm,
        "(for-each (lambda (grob)
                     (%grob-set-property-data! grob 'stencil (ly:round-filled-box '(0 . 1) '(0 . 1) 0)))
                   made)",
    );
    Some((vm, done))
}

#[test]
fn an_engine_grobs_link_is_its_event_causes_origin() {
    let Some((mut vm, _done)) = translated() else {
        return;
    };
    let roots = ["/s"];
    let paper = Paper {
        output_scale: Paper::output_scale_of(20.0),
        tree_roots: &roots,
    };
    // The link each grob's print stencil writes with `point-and-click` set
    // to `option`.
    let mut written = |option: &str| -> [Option<String>; 2] {
        eval(
            &mut vm,
            &format!("(ly:set-option 'point-and-click {option})"),
        );
        [0, 1].map(|n| {
            let grob = eval(&mut vm, &format!("(g {n})"));
            let stencil = get_print_stencil(&mut vm, grob).unwrap();
            let page = write_page(&mut vm, &stencil, &paper)
                .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
            assert_eq!(page.warnings, Vec::<String>::new());
            let (_, rest) = page.svg.split_once("<a xlink:href=\"")?;
            Some(rest[..rest.find('"').unwrap()].to_string())
        })
    };
    let link = Some(String::from("textedit:///a%20b.ly:3:4:7"));
    // The TextScript of the note links it; the one of a grob, none.
    assert_eq!(written("#t"), [link.clone(), None]);
    assert_eq!(written("'note-event"), [link.clone(), None]);
    assert_eq!(written("'(rest-event note-event)"), [link, None]);
    assert_eq!(written("'rest-event"), [None, None]);
    assert_eq!(written("#f"), [None, None]);
}

/// The packs `build.mjs` built, once; `None` without LilyPond 2.26.0.
fn packs() -> Option<&'static Path> {
    static BUILT: OnceLock<Option<PathBuf>> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            let version = Command::new("lilypond").arg("--version").output().ok()?;
            if !String::from_utf8_lossy(&version.stdout).contains("GNU LilyPond 2.26.0") {
                return None;
            }
            let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("svg-pages");
            std::fs::remove_dir_all(&dir).ok();
            std::fs::create_dir_all(&dir).unwrap();
            let run = Command::new("node")
                .arg(engine().join("tools/packs/build.mjs"))
                .arg("--out")
                .arg(&dir)
                .output()
                .expect("node runs tools/packs/build.mjs");
            assert!(
                run.status.success(),
                "tools/packs/build.mjs failed: {}",
                String::from_utf8_lossy(&run.stderr)
            );
            Some(dir)
        })
        .as_deref()
}

/// Each link of `svg` in order, as chapter 00 reads it: the file's name,
/// its line, char and column, and the `translate` of its first child.
fn links(svg: &str) -> Vec<(String, f64, f64)> {
    let mut out = Vec::new();
    let mut rest = svg;
    while let Some(at) = rest.find("xlink:href=\"textedit://") {
        rest = &rest[at + "xlink:href=\"textedit://".len()..];
        let end = rest.find('"').unwrap();
        let href = &rest[..end];
        let file = href.rsplit('/').next().unwrap().to_string();
        let after = &rest[end..];
        let group = after
            .strip_prefix("\">\n<g transform=\"translate(")
            .unwrap_or_else(|| panic!("{href}: no translate group first"));
        let close = group.find(')').unwrap();
        let (x, y) = group[..close].split_once(", ").unwrap();
        out.push((file, x.parse().unwrap(), y.parse().unwrap()));
    }
    out
}

#[test]
fn the_ten_scores_pages_written_with_lilyponds_links() {
    let Some(packs) = packs() else {
        eprintln!(
            "skipped: LilyPond 2.26.0 is not installed: the writer is not checked on the 11 pages"
        );
        return;
    };
    let refs = corpus().join("ref");
    if !TEN
        .iter()
        .all(|id| refs.join(id).join("pages.scm").exists())
    {
        eprintln!(
            "skipped: no pages.scm in {}: run npm run capture; the writer is not checked on the 11 pages",
            refs.display()
        );
        return;
    }
    let packs: Vec<Vec<u8>> = PACKS
        .iter()
        .map(|name| std::fs::read(packs.join(format!("{name}.pack"))).unwrap())
        .collect();
    let mut vm = vm(&packs);
    let (mut pages, mut linked) = (0, 0);
    for id in TEN {
        let dir = refs.join(id);
        let text = std::fs::read_to_string(dir.join("pages.scm")).unwrap();
        for line in text.lines() {
            let (name, stencil) = read(&mut vm, line);
            // Every directory a link names is a root here: only the files' names are compared.
            let roots = ["/"];
            let paper = Paper {
                output_scale: Paper::output_scale_of(20.0),
                tree_roots: &roots,
            };
            let page = write_page(&mut vm, &stencil, &paper).unwrap();
            assert_eq!(page.warnings, Vec::<String>::new(), "{id} {name}");
            // Each glyph defined once.
            let mut ids: Vec<&str> = page
                .svg
                .match_indices("<path id=\"")
                .map(|(at, _)| {
                    let id = &page.svg[at + 10..];
                    &id[..id.find('"').unwrap()]
                })
                .collect();
            let defined = ids.len();
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), defined, "{id} {name}: a glyph defined twice");
            let lilypond = std::fs::read_to_string(dir.join(&name)).unwrap();
            let theirs = links(&lilypond);
            let ours = links(&page.svg);
            assert_eq!(ours, theirs, "{id} {name}: the links");
            pages += 1;
            linked += ours.len();
            vm.collect();
        }
    }
    // One page each, T17's two; step 6 counted 245 links.
    assert_eq!((pages, linked), (11, 245));
    eprintln!("written: {pages} pages, {linked} links as LilyPond's");
}
