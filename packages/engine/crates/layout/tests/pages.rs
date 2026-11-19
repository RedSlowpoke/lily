//! The references of W3 and text (MS3 step 6) read back natively: each
//! record of `$LILY_CORPUS/ref/<id>/pages.scm` and `markups.scm` for the 10
//! scores of MS3's gate (`tools/compare/stencils.ly`, written by
//! `npm run capture`), read by the engine's reader into layout's stencils
//! and fonts (`%dump->stencil`, given the packs `tools/packs/build.mjs`
//! builds from the installed LilyPond's OTFs), has the capture's extents,
//! and written back (`%stencil->dump`) is the capture's line, character for
//! character. Skipped without LilyPond 2.26.0 or without the captures, and
//! then unverified; `tools/compare/test/stencils.test.mjs` captures them
//! afresh and reads them back from the image natively and in wasm.
//!
//! The book's `Paper_book`s (MS3 step 35): `Book::process` of a book and
//! its parts (`book.cc:197–330`), their papers scaled, parented and
//! normalized, and the items each prints in LilyPond's order; the system
//! specs and the systems of a `Paper_book` (`paper-book.cc:563–749`): the
//! titles, the headers' `breakbefore`, a markup list's lines and their
//! footnotes, each Paper_score's paper systems, a book's parts', on a
//! hand-made paper whose title procedures stand in for `titling.scm`'s;
//! and `ly:paper-get-number` and `ly:module->alist`, which `titling.scm`
//! calls; page markers' permissions and labels (MS3 step 64); a book's
//! pages (`Page_breaking::draw_page`, `make_pages`) of
//! markups, their configurations LilyPond's; `Paper_book::pages`, the
//! paper's page breaker's pages (MS3 step 62), with `page.scm`'s
//! `make-page`, `calc-printable-height` and `page-stencil` stand-ins; the
//! page breaker's and page spacer's parts on hand-made lines; and the
//! other primitives `page.scm` and
//! `titling.scm` call (D131). Without LilyPond.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use lily_engine_layout::Interval;
use lily_engine_layout::constrained_breaking::{LineDetails, LineShape};
use lily_engine_layout::font;
use lily_engine_layout::page;
use lily_engine_layout::page_breaking::{BreakPosition, PageBreaking};
use lily_engine_layout::page_spacing::{
    BAD_SPACING_PENALTY, PageSpacer, PageSpacing, SYSTEM_COUNT_OK, SYSTEM_COUNT_TOO_FEW,
    SYSTEM_COUNT_TOO_MANY, TERRIBLE_SPACING_PENALTY,
};
use lily_engine_layout::paper_book::{self, PaperBook};
use lily_engine_music::{Book, OutputDef};
use lily_engine_scheme::{Value, Vm};

/// The 10 scores of MS3's gate (README §5).
const TEN: [&str; 10] = [
    "T01", "T03", "T05", "T10", "T13", "T14", "T16", "T17", "T18", "T19",
];

/// The packs the pages draw with: Emmentaler-20 and the braces (T13).
const PACKS: [&str; 5] = [
    "emmentaler-20",
    "emmentaler-brace-0",
    "emmentaler-brace-1",
    "emmentaler-brace-2",
    "emmentaler-brace-3",
];

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

/// The packs `build.mjs` built, once; `None` without LilyPond 2.26.0.
fn packs() -> Option<&'static Path> {
    static BUILT: OnceLock<Option<PathBuf>> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            let version = Command::new("lilypond").arg("--version").output().ok()?;
            if !String::from_utf8_lossy(&version.stdout).contains("GNU LilyPond 2.26.0") {
                return None;
            }
            let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("layout-pages");
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

/// A VM with the music crate's and layout's functions, `(lily)` current,
/// given the packs.
fn vm(packs: &Path) -> Vm {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    lily_engine_layout::install(&mut vm).unwrap();
    vm.eval_text("(set-current-module (resolve-module '(lily)))", None)
        .unwrap();
    // The font metrics in an expression.
    vm.eval_text(
        "(define (fonts-in x)
           (cond ((ly:font-metric? x) 1)
                 ((pair? x) (+ (fonts-in (car x)) (fonts-in (cdr x))))
                 (else 0)))",
        None,
    )
    .unwrap();
    for pack in PACKS {
        let bytes = std::fs::read(packs.join(format!("{pack}.pack"))).unwrap();
        font::put_pack(&mut vm, &bytes).unwrap_or_else(|e| panic!("{pack}: {}", e.text(vm.heap())));
    }
    vm
}

/// Reads `line`, a record whose stencil's data starts at its item `at`,
/// into a stencil and back: whether its extents are the capture's, the
/// font metrics in its expression, and the record written again, as
/// `write` writes it.
fn read_back(vm: &mut Vm, line: &str, at: usize) -> (bool, i64, String) {
    let source = format!(
        "(let* ((r '{line}) (d (list-tail r {at})))
           (if (car d)
               (let ((s (apply %dump->stencil d)))
                 (cons* (and (equal? (ly:stencil-extent s 0) (car d))
                             (equal? (ly:stencil-extent s 1) (cadr d)))
                        (fonts-in (ly:stencil-expr s))
                        (append (list-head r {at}) (%stencil->dump s))))
               (cons* #t 0 r)))"
    );
    let value = vm
        .eval_text(&source, None)
        .unwrap_or_else(|e| panic!("{}: {}", &line[..line.len().min(80)], e.text(vm.heap())));
    let heap = vm.heap();
    let rest = heap.cdr(value).unwrap();
    let mut out = String::new();
    heap.write(heap.cdr(rest).unwrap(), &mut out);
    let fonts = heap.car(rest).and_then(|n| n.as_fixnum()).unwrap();
    (
        heap.car(value) == Some(lily_engine_scheme::Value::TRUE),
        fonts,
        out,
    )
}

#[test]
fn the_ten_scores_pages_and_markups_read_back_whole() {
    let Some(packs) = packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the references are not read back");
        return;
    };
    let refs = corpus().join("ref");
    if !TEN
        .iter()
        .all(|id| refs.join(id).join("pages.scm").exists())
    {
        eprintln!(
            "skipped: no pages.scm in {}: run npm run capture; the references are not read back",
            refs.display()
        );
        return;
    }
    let mut vm = vm(packs);
    let (mut pages, mut markups, mut fonts) = (0, 0, 0);
    for id in TEN {
        for (file, head, at) in [("pages.scm", "(page ", 3), ("markups.scm", "(markup ", 5)] {
            let text = std::fs::read_to_string(refs.join(id).join(file)).unwrap();
            for line in text.lines() {
                assert!(
                    line.starts_with(head),
                    "{id} {file}: {}",
                    &line[..line.len().min(80)]
                );
                let (extents, n, back) = read_back(&mut vm, line, at);
                // Each font of a pack in a page read as a font metric; a
                // markup's chain may name fonts too, which stay data.
                let written = line.matches("(%font \"").count();
                if at == 3 {
                    assert_eq!(n as usize, written, "{id} {file}: the fonts of a page");
                } else {
                    assert!(n as usize <= written, "{id} {file}: the fonts of a markup");
                }
                assert!(extents, "{id} {file}: the extents of {}", &line[..80]);
                assert!(
                    back == line,
                    "{id} {file}: read back, the record differs:\n  capture: {}\n  engine:  {}",
                    &line[..line.len().min(400)],
                    &back[..back.len().min(400)]
                );
                fonts += n;
                if at == 3 { pages += 1 } else { markups += 1 }
            }
        }
    }
    // 11 pages: one each, T17's two.
    assert_eq!(pages, 11, "the pages");
    assert!(markups > 0 && fonts > 0);
    eprintln!("read back: {pages} pages and {markups} markups, {fonts} fonts of packs");
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

#[test]
fn the_data_form_without_lilypond() {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    lily_engine_layout::install(&mut vm).unwrap();
    vm.eval_text("(set-current-module (resolve-module '(lily)))", None)
        .unwrap();
    // A link, Pango's font and text stay as they are; the extents are kept.
    let data = r#"((-0.0 . 2.5) (+inf.0 . -inf.0) (combine-stencil (grob-cause "textedit:///a.ly:1:2:3" (draw-line 0.1 0 0 1.0 0)) (grob-cause #f ()) (utf-8-string "C059, 3.865234375" "x" (glyph-string (%font #f "unknown" 1.0) "C059-Roman" 3.865234375 #f ((0.5 (0.0 . 1.0) 0.0 0.0 73 "x")) "C059-Roman.otf" 0 "x" (0)))))"#;
    assert_eq!(
        run(
            &mut vm,
            &format!("(%stencil->dump (apply %dump->stencil '{data}))")
        ),
        (String::new(), data.to_string())
    );
    assert_eq!(
        run(
            &mut vm,
            "(ly:stencil-extent (%dump->stencil '(-0.0 . 2.5) '(+inf.0 . -inf.0) '()) 1)"
        ),
        (String::new(), "(+inf.0 . -inf.0)".to_string())
    );
    // A font of no pack, and extents that are none.
    assert_eq!(
        run(
            &mut vm,
            r#"(%dump->stencil '(0 . 1) '(0 . 1) '(named-glyph (%font "emmentaler-20" "Emmentaler-20" 0.5) "x"))"#
        ),
        (
            "misc-error".to_string(),
            "cannot find font 'emmentaler-20'".to_string()
        )
    );
    assert_eq!(
        run(&mut vm, "(%dump->stencil 0 '(0 . 1) '())").0,
        "wrong-type-arg"
    );
    assert_eq!(run(&mut vm, "(%stencil->dump 0)").0, "wrong-type-arg");
}

/// A VM with the music crate's and layout's functions, `(lily)` current,
/// without packs.
fn bare_vm() -> Vm {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    lily_engine_layout::install(&mut vm).unwrap();
    vm.eval_text("(set-current-module (resolve-module '(lily)))", None)
        .unwrap();
    vm
}

/// `value` as `write` writes it.
fn written(vm: &Vm, value: Value) -> String {
    let mut out = String::new();
    vm.heap().write(value, &mut out);
    out
}

/// The value `name` has in `def`'s own scope, written, or `-` if it has
/// none there.
fn own(vm: &mut Vm, def: Value, name: &str) -> String {
    let scope = vm.heap().foreign::<OutputDef>(def).unwrap().scope;
    match vm.module_value(scope, name) {
        Some(v) => written(vm, v),
        None => "-".to_string(),
    }
}

/// `name` looked up in `def` and its parents, written.
fn lookup(vm: &mut Vm, def: Value, name: &str) -> String {
    vm.define("%def", def);
    let v = vm
        .eval_text(&format!("(ly:output-def-lookup %def '{name} '-)"), None)
        .unwrap();
    written(vm, v)
}

/// A paper named `name` with the variables `Book::process` reads of
/// `$defaultpaper`: its kind, `output-scale` `scale`, `mm` among the
/// `dimension-variables`, and what `Output_def::normalize` reads, on a
/// paper 100 wide with the default margins 10 and 5.
fn book_paper(vm: &mut Vm, name: &str, scale: &str) -> Value {
    vm.eval_text(
        &format!(
            "(define-public {name} (ly:make-output-def))
             (for-each (lambda (e) (ly:output-def-set-variable! {name} (car e) (cdr e)))
                       '((output-def-kind . paper) (output-scale . {scale}) (mm . 1)
                         (dimension-variables . (mm)) (paper-width . 100)
                         (left-margin-default-scaled . 10) (right-margin-default-scaled . 5)
                         (outer-margin-default-scaled . 12) (inner-margin-default-scaled . 8)))
             {name}"
        ),
        None,
    )
    .unwrap()
}

#[test]
fn a_book_is_processed_into_its_paper_books_and_its_parts_in_lilypond_s_order() {
    let mut vm = bare_vm();
    let paper = book_paper(&mut vm, "book-paper", "2.0");
    let part_paper = book_paper(&mut vm, "part-paper", "4");
    let default = book_paper(&mut vm, "default-paper", "8.0");
    let header = vm.make_module(&["lily"]).unwrap();
    let book = vm.heap_mut().wrap(Book::new(paper, header));
    vm.define("book", book);
    let plain = vm.heap_mut().wrap(Book::new(Value::FALSE, Value::FALSE));
    vm.define("plain-part", plain);
    let own_part = vm.heap_mut().wrap(Book::new(part_paper, Value::FALSE));
    vm.define("own-part", own_part);
    vm.eval_text(
        "(ly:book-add-score! book 's1)
         (ly:book-add-score! book 's2)
         (ly:book-add-score! plain-part 's3)
         (ly:book-add-bookpart! book plain-part)
         (ly:book-add-score! book 's4)
         (ly:book-add-score! own-part 's5)
         (ly:book-add-bookpart! book own-part)
         (ly:book-add-score! book 's6)",
        None,
    )
    .unwrap();
    let mut out = Vec::new();
    let top = paper_book::process(&mut vm, book, default, &mut out)
        .unwrap()
        .expect("a book's Paper_book");
    // The scores in the order they were added, the book's own before,
    // between and after its parts each in a part of their own.
    let names: Vec<String> = out.iter().map(|&(s, _)| written(&vm, s)).collect();
    assert_eq!(names, ["s1", "s2", "s3", "s4", "s5", "s6"]);
    let b = vm.heap().foreign::<PaperBook>(top).unwrap();
    assert!(b.print_bookparts);
    assert_eq!(b.header, header);
    assert_eq!(b.parent, Value::FALSE);
    let (top_paper, parts) = (b.paper, b.print_elements.clone());
    assert_eq!(parts.len(), 5, "s1 s2 | plain-part | s4 | own-part | s6");
    let of: Vec<usize> = out
        .iter()
        .map(|&(_, pb)| parts.iter().position(|&p| p == pb).unwrap())
        .collect();
    assert_eq!(of, [0, 0, 1, 2, 3, 4]);
    // The book's: its paper scaled by its own scale, no parent, not
    // normalized (it prints its parts).
    assert_eq!(lookup(&mut vm, top_paper, "mm"), "0.5");
    assert_eq!(lookup(&mut vm, top_paper, "cloned"), "#t");
    assert_eq!(
        vm.heap().foreign::<OutputDef>(top_paper).unwrap().parent,
        None
    );
    assert_eq!(own(&mut vm, top_paper, "line-width"), "-");
    for (i, &part) in parts.iter().enumerate() {
        let p = vm.heap().foreign::<PaperBook>(part).unwrap();
        assert_eq!(p.parent, top, "part {i}");
        assert!(!p.print_bookparts && p.print_elements.is_empty());
        let pp = p.paper;
        // Each part's paper parented to the book's `Paper_book`'s, so
        // `top_paper` is that; and normalized: 100 less the margins.
        assert_eq!(
            vm.heap().foreign::<OutputDef>(pp).unwrap().parent,
            Some(top_paper)
        );
        assert_eq!(paper_book::top_paper(vm.heap(), part), top_paper);
        assert_eq!(own(&mut vm, pp, "line-width"), "85.0", "part {i}");
        // A part's own paper is scaled by its own scale; a new one has no
        // `mm` and finds the book's scaled one.
        let mm = if part == parts[3] { "0.25" } else { "0.5" };
        assert_eq!(lookup(&mut vm, pp, "mm"), mm, "part {i}");
    }
    // A book of no paper and no parts: the default paper's `Paper_book`,
    // normalized, printing its scores itself.
    let alone = vm.heap_mut().wrap(Book::new(Value::FALSE, Value::FALSE));
    vm.define("alone", alone);
    vm.eval_text("(ly:book-add-score! alone 's7)", None)
        .unwrap();
    let mut out = Vec::new();
    let printed = paper_book::process(&mut vm, alone, default, &mut out)
        .unwrap()
        .unwrap();
    assert_eq!(out, [(vm.eval_text("'s7", None).unwrap(), printed)]);
    let p = vm.heap().foreign::<PaperBook>(printed).unwrap().paper;
    assert_eq!(lookup(&mut vm, p, "mm"), "0.125");
    assert_eq!(own(&mut vm, p, "line-width"), "85.0");
    // What a compile adds, in order: `add_score` and `add_bookpart`.
    paper_book::add_score(vm.heap_mut(), printed, Value::TRUE);
    paper_book::add_bookpart(vm.heap_mut(), printed, top);
    let p = vm.heap().foreign::<PaperBook>(printed).unwrap();
    assert_eq!(p.print_elements, [Value::TRUE, top]);
    assert!(p.print_bookparts);
    // No book: no `Paper_book`.
    assert_eq!(
        paper_book::process(&mut vm, Value::FALSE, default, &mut Vec::new()).unwrap(),
        None
    );
}

/// The paper of a book whose `Paper_book`'s systems are asked for: its
/// `line-width`, and stand-ins for what `paper-defaults-init.ly` and
/// `titling.scm` give it, which [`systems_of_a_book`] checks the calls of:
/// `book-title` draws `(draw-line 0.1 0 0 1 n)` of `n` scopes that bind
/// `title`; `score-title`, `(draw-line 0.2 0 0 1 n)` of `n` scopes whose
/// first binds `piece`;
/// `layout-extract-page-properties` reads `line-width` by
/// `ly:paper-get-number`; a markup list is `(lines e …)`, a stencil of
/// each `e` with the properties it is given kept in `%props`. The
/// stencils' heads registered, as `output-lib.scm` registers them.
fn titled_paper(vm: &mut Vm) -> Value {
    vm.eval_text(
        "(for-each ly:register-stencil-expression
                   '(draw-line footnote combine-stencil translate-stencil))
         (define (binds? m s) (module-ref m s #f))
         (define-public titled (ly:make-output-def))
         (ly:output-def-set-variable! titled 'line-width 80)
         (ly:output-def-set-variable! titled 'book-title
           (lambda (paper scopes)
             (if (and (pair? scopes) (binds? (car scopes) 'title))
                 (ly:make-stencil (list 'draw-line 0.1 0 0 1 (length scopes)) '(0 . 10) '(-2 . 3))
                 (ly:make-stencil '()))))
         (ly:output-def-set-variable! titled 'score-title
           (lambda (paper scopes)
             (if (and (pair? scopes) (binds? (car scopes) 'piece))
                 (ly:make-stencil (list 'draw-line 0.2 0 0 1 (length scopes)) '(0 . 5) '(-1 . 1))
                 (ly:make-stencil '()))))
         (ly:output-def-set-variable! titled 'book-title-properties '((is-book-title . #t)))
         (ly:output-def-set-variable! titled 'score-title-properties '((is-book-title . #f)))
         (define-public (layout-extract-page-properties layout)
           (list (list (cons 'line-width (ly:paper-get-number layout 'line-width)))))
         (define-public %props #f)
         (define-public (markup-list? x) (and (pair? x) (eq? (car x) 'lines)))
         (define-public (interpret-markup-list paper props ml)
           (set! %props props)
           (map (lambda (e) (ly:make-stencil e '(0 . 1) '(0 . 1))) (cdr ml)))
         titled",
        None,
    )
    .unwrap()
}

/// A header binding `bindings`, a module as `\header` makes.
fn header(vm: &mut Vm, bindings: &[(&str, Value)]) -> Value {
    let m = vm.make_module(&["lily"]).unwrap();
    for &(name, v) in bindings {
        vm.module_define(m, name, v);
    }
    m
}

/// What each of `specs` is, in order: a Paper_score by its place in
/// `pscores`, a paper system by its stencil's expression and the
/// properties `names` it has, mutable or immutable, `'()` too.
fn described(vm: &mut Vm, specs: Value, pscores: &[Value], names: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut s = specs;
    while let (Some(spec), Some(rest)) = (vm.heap().car(s), vm.heap().cdr(s)) {
        if let Some(i) = pscores.iter().position(|&p| p == spec) {
            out.push(format!("ps{}", i + 1));
        } else if vm.heap().foreign::<lily_engine_music::Prob>(spec).is_some() {
            vm.define("%spec", spec);
            let expr = vm
                .eval_text("(ly:stencil-expr (ly:prob-property %spec 'stencil))", None)
                .unwrap();
            let mut line = written(vm, expr);
            for name in names {
                let v = vm
                    .eval_text(
                        &format!(
                            "(or (assq '{name} (ly:prob-mutable-properties %spec))
                                 (assq '{name} (ly:prob-immutable-properties %spec)))"
                        ),
                        None,
                    )
                    .unwrap();
                if let Some(v) = vm.heap().cdr(v) {
                    line.push_str(&format!(" {name}={}", written(vm, v)));
                }
            }
            out.push(line);
        } else {
            out.push(written(vm, spec));
        }
        s = rest;
    }
    out
}

#[test]
fn systems_of_a_book() {
    let mut vm = bare_vm();
    let paper = titled_paper(&mut vm);
    let layout = vm.eval_text("(ly:make-output-def)", None).unwrap();
    vm.heap_mut().root(layout);
    let t = vm.heap_mut().string("T");
    let book_header = header(&mut vm, &[("title", t)]);
    let a = vm.heap_mut().string("A");
    let b = vm.heap_mut().string("B");
    let h1 = header(&mut vm, &[("piece", a), ("breakbefore", Value::FALSE)]);
    let h2 = header(&mut vm, &[("piece", b), ("breakbefore", Value::TRUE)]);
    let lines = vm
        .eval_text(
            "'(lines (combine-stencil (footnote a b) (translate-stencil (1 . 2) (footnote c))) (draw-line 0.3 0 0 1 2) (draw-line 0.3 0 0 1 3))",
            None,
        )
        .unwrap();
    // Three Paper_scores with their paper systems made: the first two,
    // the others none.
    let mut pscores = Vec::new();
    for made in ["(vector 'sys1 'sys2)", "(vector)", "(vector)"] {
        let mut ps = lily_engine_layout::paper_score::PaperScore::new(layout);
        ps.paper_systems = vm.eval_text(made, None).unwrap();
        let ps = vm.heap_mut().wrap(ps);
        vm.heap_mut().root(ps);
        pscores.push(ps);
    }

    let book = vm.heap_mut().wrap(PaperBook {
        paper,
        parent: Value::FALSE,
        header: book_header,
        header_0: Value::NULL,
        pages: Value::FALSE,
        systems: Value::FALSE,
        print_elements: Vec::new(),
        print_bookparts: false,
        performances: Vec::new(),
    });
    vm.heap_mut().root(book);
    for e in [h1, pscores[0], lines, h2, pscores[1], pscores[2]] {
        paper_book::add_score(vm.heap_mut(), book, e);
    }

    let specs = paper_book::get_system_specs(&mut vm, book)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    vm.heap_mut().root(specs);
    let names = [
        "is-book-title",
        "page-break-permission",
        "line-break-permission",
        "page-turn-permission",
        "first-markup-line",
        "last-markup-line",
        "tight-spacing",
        "is-title",
        "footnotes",
    ];
    // The book title, its `breakbefore` #f from the first score's header;
    // each score's title before it, of its header and the book's; a line
    // of the markup list each, the last line before the second score
    // forced by its header's `breakbefore` #t; the third score with no
    // header has no title.
    assert_eq!(
        described(&mut vm, specs, &pscores, &names),
        [
            "(translate-stencil (0.0 . -3.0) (draw-line 0.1 0 0 1 1)) is-book-title=#t page-break-permission=()".to_string(),
            "(translate-stencil (0.0 . -1.0) (draw-line 0.2 0 0 1 2)) is-book-title=#f".to_string(),
            "ps1".to_string(),
            "(combine-stencil (footnote a b) (translate-stencil (1 . 2) (footnote c))) \
             page-break-permission=allow page-turn-permission=allow first-markup-line=#t \
             last-markup-line=#f is-title=#t footnotes=((a b) (c))"
                .to_string(),
            "(draw-line 0.3 0 0 1 2) page-break-permission=allow page-turn-permission=allow \
             first-markup-line=#f last-markup-line=#f tight-spacing=#t is-title=#t footnotes=()"
                .to_string(),
            "(draw-line 0.3 0 0 1 3) page-break-permission=force line-break-permission=force \
             page-turn-permission=allow first-markup-line=#f last-markup-line=#t \
             tight-spacing=#t is-title=#t footnotes=()"
                .to_string(),
            "(translate-stencil (0.0 . -1.0) (draw-line 0.2 0 0 1 2)) is-book-title=#f".to_string(),
            "ps2".to_string(),
            "ps3".to_string(),
        ]
    );
    // The page properties the markup list was interpreted with: the
    // paper's `line-width` by `ly:paper-get-number`.
    let props = vm.eval_text("%props", None).unwrap();
    assert_eq!(written(&vm, props), "(((line-width . 80.0)))");
    // The first header found.
    assert_eq!(vm.heap().foreign::<PaperBook>(book).unwrap().header_0, h1);
    // The titles aligned on their tops: the book's moved down by 3.
    vm.define("%specs", specs);
    let y = vm
        .eval_text(
            "(ly:stencil-extent (ly:prob-property (car %specs) 'stencil) 1)",
            None,
        )
        .unwrap();
    assert_eq!(written(&vm, y), "(-5.0 . 0.0)");

    // The systems: the specs, each Paper_score's paper systems in its
    // place, made once.
    let systems =
        paper_book::systems(&mut vm, book).unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    let got = described(&mut vm, systems, &pscores, &[]);
    assert_eq!(
        got,
        [
            "(translate-stencil (0.0 . -3.0) (draw-line 0.1 0 0 1 1))",
            "(translate-stencil (0.0 . -1.0) (draw-line 0.2 0 0 1 2))",
            "sys1",
            "sys2",
            "(combine-stencil (footnote a b) (translate-stencil (1 . 2) (footnote c)))",
            "(draw-line 0.3 0 0 1 2)",
            "(draw-line 0.3 0 0 1 3)",
            "(translate-stencil (0.0 . -1.0) (draw-line 0.2 0 0 1 2))",
        ]
    );
    assert_eq!(paper_book::systems(&mut vm, book).unwrap(), systems);

    // A book printing its parts: their systems in order, each part's its
    // own; a part with a header and no title prints its score's alone.
    let part_of = |vm: &mut Vm, elements: &[Value]| {
        let p = vm.heap_mut().wrap(PaperBook {
            paper,
            parent: book,
            header: Value::FALSE,
            header_0: Value::NULL,
            pages: Value::FALSE,
            systems: Value::FALSE,
            print_elements: elements.to_vec(),
            print_bookparts: false,
            performances: Vec::new(),
        });
        vm.heap_mut().root(p);
        p
    };
    let p1 = part_of(&mut vm, &[pscores[0]]);
    let p2 = part_of(&mut vm, &[h1, pscores[2]]);
    let top = part_of(&mut vm, &[]);
    paper_book::add_bookpart(vm.heap_mut(), top, p1);
    paper_book::add_bookpart(vm.heap_mut(), top, p2);
    let systems = paper_book::systems(&mut vm, top).unwrap();
    assert_eq!(
        described(&mut vm, systems, &pscores, &[]),
        [
            "sys1",
            "sys2",
            "(translate-stencil (0.0 . -1.0) (draw-line 0.2 0 0 1 1))"
        ]
    );
    let p1_systems = vm.heap().foreign::<PaperBook>(p1).unwrap().systems;
    assert_eq!(written(&vm, p1_systems), "(sys1 sys2)");
    assert_eq!(vm.heap().foreign::<PaperBook>(p2).unwrap().header_0, h1);
}

/// Page markers among a book's items (MS3 step 64): a permission sets
/// the spec before it, `'()` taking it away, nothing before the first
/// spec; the labels since the last spec go to the next one, the last
/// first, appended to its own (`set_labels`); and the markers' and paper
/// systems' predicates.
#[test]
fn page_markers_set_permissions_and_labels() {
    let mut vm = bare_vm();
    let paper = titled_paper(&mut vm);
    let book = vm.heap_mut().wrap(PaperBook {
        paper,
        parent: Value::FALSE,
        header: Value::FALSE,
        header_0: Value::NULL,
        pages: Value::FALSE,
        systems: Value::FALSE,
        print_elements: Vec::new(),
        print_bookparts: false,
        performances: Vec::new(),
    });
    vm.heap_mut().root(book);
    let items = vm
        .eval_text(
            "(list (ly:make-page-permission-marker 'page-break-permission 'force)
                   (ly:make-page-label-marker 'a)
                   (ly:make-page-label-marker 'b)
                   '(lines (draw-line 0.3 0 0 1 1) (draw-line 0.3 0 0 1 2))
                   (ly:make-page-permission-marker 'page-break-permission 'force)
                   (ly:make-page-permission-marker 'page-turn-permission '())
                   (ly:make-page-label-marker 'c)
                   '(lines (draw-line 0.3 0 0 1 3)))",
            None,
        )
        .unwrap();
    vm.heap_mut().root(items);
    let mut list = items;
    while let (Some(item), Some(rest)) = (vm.heap().car(list), vm.heap().cdr(list)) {
        paper_book::add_score(vm.heap_mut(), book, item);
        list = rest;
    }
    let specs = paper_book::get_system_specs(&mut vm, book)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    vm.heap_mut().root(specs);
    let names = ["page-break-permission", "page-turn-permission", "labels"];
    assert_eq!(
        described(&mut vm, specs, &[], &names),
        [
            "(draw-line 0.3 0 0 1 1) page-break-permission=allow page-turn-permission=allow \
             labels=(b a)",
            "(draw-line 0.3 0 0 1 2) page-break-permission=force page-turn-permission=()",
            "(draw-line 0.3 0 0 1 3) page-break-permission=allow page-turn-permission=allow \
             labels=(c)",
        ]
    );
    vm.define("%specs", specs);
    vm.define("%items", items);
    assert_eq!(
        run(
            &mut vm,
            "(list (ly:page-marker? (car %items)) (ly:page-marker? (car %specs))
                   (ly:paper-system? (car %specs)) (ly:paper-system? (car %items))
                   (ly:paper-system? (ly:make-prob 'page '())))"
        ),
        (String::new(), "(#t #f #t #f #f)".to_string())
    );
    assert_eq!(
        run(&mut vm, "(ly:make-page-label-marker \"a\")").0,
        "wrong-type-arg"
    );
    assert_eq!(
        run(&mut vm, "(ly:make-page-permission-marker 1 'force)").0,
        "wrong-type-arg"
    );
}

#[test]
fn paper_get_number_and_module_alist() {
    let mut vm = bare_vm();
    titled_paper(&mut vm);
    assert_eq!(
        run(&mut vm, "(ly:paper-get-number titled 'line-width)"),
        (String::new(), "80.0".to_string())
    );
    // `scm_to_double`'s error, from no procedure.
    assert_eq!(
        run(
            &mut vm,
            "(ly:paper-get-number titled 'book-title-properties)"
        ),
        (
            "wrong-type-arg".to_string(),
            "Wrong type (expecting real number): ((is-book-title . #t))".to_string()
        )
    );
    assert_eq!(
        run(&mut vm, "(ly:paper-get-number titled 'nothing)").0,
        "wrong-type-arg"
    );
    assert_eq!(
        run(&mut vm, "(ly:paper-get-number 1 'x)").0,
        "wrong-type-arg"
    );
    // A module's own bindings, not those it uses.
    let m = vm.make_module(&["lily"]).unwrap();
    vm.module_define(m, "a", Value::TRUE);
    vm.module_define(m, "b", Value::FALSE);
    vm.define("%m", m);
    assert_eq!(
        run(
            &mut vm,
            "(sort (ly:module->alist %m) (lambda (x y) (string<? (symbol->string (car x)) (symbol->string (car y)))))"
        ),
        (String::new(), "((a . #t) (b . #f))".to_string())
    );
    assert_eq!(run(&mut vm, "(ly:module->alist 1)").0, "wrong-type-arg");
}

/// A paper with `$defaultpaper`'s variables that the pages of markups
/// read, as LilyPond 2.26.0 prints them, with `markup-markup-spacing`
/// stretchable by 20; the pages of `(lily page)`'s `make-page` stand-in,
/// each a prob of `%blank-pages` given what `make-page` gives it and an
/// empty head and foot (no header or footer markup), the arguments it is
/// called with kept in `%made`; `number-format` the type and the number;
/// `ly:warning`'s messages kept in `%warnings`.
fn page_paper(vm: &mut Vm, pages: usize) -> Value {
    let blank: Vec<Value> = (0..pages)
        .map(|_| {
            let heap = vm.heap_mut();
            let ty = heap.intern("page");
            let p = lily_engine_music::make_prob(heap, ty, Value::NULL);
            heap.root(p);
            p
        })
        .collect();
    let blank = vm.heap_mut().list(&blank);
    vm.heap_mut().root(blank);
    vm.define("%blank-pages", blank);
    vm.eval_text(
        "(for-each ly:register-stencil-expression '(round-filled-box num f))
         (define-public %made '())
         (define-public %warnings '())
         (define-public (ly:warning msg . args)
           (set! %warnings (cons (if (null? args) msg (apply format #f msg args)) %warnings)))
         (define-public (number-format type n) (string-append (symbol->string type) (number->string n)))
         (define-public paged (ly:make-output-def))
         (for-each (lambda (e) (ly:output-def-set-variable! paged (car e) (cdr e)))
                   '((paper-height . 169.00937007874015) (top-margin . 5.690551181102362)
                     (bottom-margin . 5.690551181102362) (line-width . 100)
                     (top-markup-spacing . ((basic-distance . 4) (minimum-distance . 0) (padding . 1)))
                     (markup-markup-spacing . ((stretchability . 20) (basic-distance . 1) (padding . 0.5)))
                     (last-bottom-spacing . ((basic-distance . 1) (minimum-distance . 0) (padding . 1)
                                             (stretchability . 30)))
                     (top-system-spacing . ((basic-distance . 6) (minimum-distance . 0) (padding . 1)))
                     (score-markup-spacing . ((basic-distance . 12) (padding . 0.5) (stretchability . 60)))
                     (ragged-bottom . #f) (ragged-last-bottom . #t) (first-page-number . 1)
                     (page-number-type . arabic) (reset-footnotes-on-new-page . #t)
                     (footnote-padding . 0.5) (footnote-footer-padding . 0.5)))
         (ly:output-def-set-variable! paged 'footnote-numbering-function
           (lambda (n) (string-append \"n\" (number->string n))))
         (define-public %blank #f)
         (set! %blank %blank-pages)
         paged",
        None,
    )
    .unwrap();
    vm.eval_text(
        "(define-module (lily page))
         (use-modules (lily))
         (define-public (make-page paper-book page-number last)
           (let ((page (if (pair? %blank) (car %blank) (ly:make-prob 'page '()))))
             (if (pair? %blank) (set! %blank (cdr %blank)))
             (set! %made (cons (list page-number last) %made))
             (for-each (lambda (e) (ly:prob-set-property! page (car e) (cdr e)))
                       `((paper-book . ,paper-book) (page-number . ,page-number)
                         (is-bookpart-last-page . ,last) (paper-height . 169.00937007874015)
                         (head-stencil . ,(ly:make-stencil '())) (foot-stencil . ,(ly:make-stencil '()))))
             page))
         ;; page.scm's: the paper height less the margins, the head and
         ;; foot stencils being empty.
         (define-public (calc-printable-height page)
           (- (ly:prob-property page 'paper-height) 5.690551181102362 5.690551181102362))
         (set-current-module (resolve-module '(lily)))
         (ly:output-def-set-variable! paged 'page-breaking ly:optimal-breaking)",
        None,
    )
    .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    vm.eval_text("paged", None).unwrap()
}

/// A markup's line as `get_system_specs` makes it: a paper system whose
/// stencil is a box 10 wide from `y0` to `y1`.
fn markup_line(vm: &mut Vm, y0: f64, y1: f64) -> Value {
    let ps = lily_engine_layout::system::make_paper_system(vm.heap_mut(), Value::NULL);
    vm.heap_mut().root(ps);
    vm.define("%line", ps);
    vm.eval_text(
        &format!(
            "(ly:prob-set-property! %line 'stencil
               (ly:make-stencil '(round-filled-box 0 10 {} {y1:?} 0) '(0 . 10) '({y0:?} . {y1:?})))
             (ly:prob-set-property! %line 'is-title #t)",
            -y0
        ),
        None,
    )
    .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    ps
}

/// The pages `make_pages` makes of markup lines from `extents` on a
/// [`page_paper`], `lines_per_page` on each: their configurations, what
/// `make-page` was called with and the warnings.
fn markup_pages(extents: &[(f64, f64)], lines_per_page: &[usize]) -> (Vec<String>, String, String) {
    let mut vm = bare_vm();
    let paper = page_paper(&mut vm, lines_per_page.len());
    let lines: Vec<Value> = extents
        .iter()
        .map(|&(y0, y1)| markup_line(&mut vm, y0, y1))
        .collect();
    let systems = vm.heap_mut().list(&lines);
    vm.heap_mut().root(systems);
    let book = vm.heap_mut().wrap(PaperBook {
        paper,
        parent: Value::FALSE,
        header: Value::NULL,
        header_0: Value::NULL,
        pages: Value::FALSE,
        systems: Value::FALSE,
        print_elements: Vec::new(),
        print_bookparts: false,
        performances: Vec::new(),
    });
    vm.heap_mut().root(book);
    let pb = PageBreaking::for_pages(&mut vm, book);
    let pages = page::make_pages(&mut vm, &pb, lines_per_page, systems)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    vm.define("%pages", pages);
    let configs = vm
        .eval_text(
            "(map (lambda (p) (ly:prob-property p 'configuration)) %pages)",
            None,
        )
        .unwrap();
    let mut out = Vec::new();
    let mut c = configs;
    while let (Some(config), Some(rest)) = (vm.heap().car(c), vm.heap().cdr(c)) {
        out.push(written(&vm, config));
        c = rest;
    }
    let made = vm.eval_text("(reverse %made)", None).unwrap();
    let warnings = vm.eval_text("(reverse %warnings)", None).unwrap();
    (out, written(&vm, made), written(&vm, warnings))
}

/// The pages of markups' lines (MS3 step 35): `make_pages` and
/// `draw_page` on the page-layout problem with a paper and a page, the
/// configurations those of LilyPond 2.26.0 for the same markups on the
/// same paper (`\markup \filled-box`, each page's `configuration` printed
/// by `page-post-process`, the pages as LilyPond broke them). The last
/// page, ragged as the last only, is spaced by the force of the page
/// before (`fixed_force_solution`): the second markup of the first case
/// is 30.85 below the first, where the ragged solution puts it 20.5
/// below; one that does not fit at that force is spaced as a page that
/// is not ragged; an over-full one is compressed, with LilyPond's two
/// warnings.
#[test]
fn pages_of_markups_are_lilypond_s() {
    // pages2.ly: two pages of two; the last at the first's force.
    let (configs, made, warnings) = markup_pages(
        &[(0.0, 60.0), (-5.0, 50.0), (0.0, 80.0), (0.0, 20.0)],
        &[2, 2],
    );
    assert_eq!(configs, ["(61.0 111.5)", "(81.0 111.85130708661416)"]);
    assert_eq!(made, "((1 #f) (2 #t))");
    assert_eq!(warnings, "()");
    // pages3.ly: a page break after two.
    let (configs, _, _) = markup_pages(
        &[
            (0.0, 10.0),
            (0.0, 5.0),
            (0.0, 50.0),
            (0.0, 50.0),
            (0.0, 40.0),
        ],
        &[2, 3],
    );
    assert_eq!(
        configs,
        [
            "(15.231723534558178 72.39034120734907)",
            "(51.0 101.5 142.0)"
        ]
    );
    // pages5.ly: a page of one, then seven that the first's force would
    // not hold: stretched as a page that is not ragged.
    let mut extents = vec![(0.0, 5.0)];
    extents.extend([(0.0, 15.0); 7]);
    let (configs, _, _) = markup_pages(&extents, &[1, 7]);
    assert_eq!(
        configs,
        [
            "(21.956266790180635)",
            "(16.0 34.950435695538054 53.90087139107611 72.85130708661416 \
             91.80174278215222 110.75217847769028 129.70261417322834)"
        ]
    );
    // pages6.ly: a markup taller than the page.
    let (configs, _, warnings) = markup_pages(&[(0.0, 30.0), (0.0, 200.0)], &[1, 1]);
    assert_eq!(configs, ["(31.0)", "(201.0)"]);
    assert_eq!(
        warnings,
        r#"("compressing over-full page by 44.4 staff-spaces" "page 2 has been compressed")"#
    );
}

/// `make_pages`' footnotes and labels (MS3 step 35): a markup line's
/// footnotes numbered on from the page before when the top paper is not
/// `reset-footnotes-on-new-page`, from 0 on each page when it is, each
/// page's foot given its lines' footnotes; the lines' `labels` kept with
/// their pages in the top paper's tables, in the lines' order, the page
/// count over the book's parts kept; and no pages of no systems. The
/// numbers are markups of a stand-in command `num`.
#[test]
fn pages_number_footnotes_and_keep_labels() {
    for (reset, numbers) in [
        ("#f", "((num 0) (num 1) (num 2))"),
        ("#t", "((num 0) (num 1) (num 0))"),
    ] {
        let mut vm = bare_vm();
        let paper = page_paper(&mut vm, 2);
        vm.eval_text(
            &format!(
                "(define-public (num-markup layout props n)
                   (ly:make-stencil (list 'num n) '(0 . 1) '(0 . 1)))
                 (define-public (markup-command-signature p) (eq? p num-markup))
                 (define-public (markup-list-function? p) #f)
                 (ly:output-def-set-variable! paged 'footnote-numbering-function
                   (lambda (n) (list num-markup n)))
                 (ly:output-def-set-variable! paged 'reset-footnotes-on-new-page {reset})
                 (ly:output-def-set-variable! paged 'absolute-page-counter 4)
                 (ly:output-def-set-variable! paged 'label-page-table '((old . 1)))"
            ),
            None,
        )
        .unwrap();
        let mut lines = Vec::new();
        for (key, height, labels) in [("a", 1, "(l2 l1)"), ("b", 2, "()"), ("c", 3, "(l3)")] {
            let line = markup_line(&mut vm, 0.0, 10.0);
            vm.define("%line", line);
            vm.eval_text(
                &format!(
                    "(ly:prob-set-property! %line 'footnotes
                       `(({key} #t ,(ly:make-stencil '(f {key}) '(0 . 5) '(0 . {height})))))
                     (ly:prob-set-property! %line 'labels '{labels})"
                ),
                None,
            )
            .unwrap();
            lines.push(line);
        }
        let systems = vm.heap_mut().list(&lines);
        vm.heap_mut().root(systems);
        let book = vm.heap_mut().wrap(PaperBook {
            paper,
            parent: Value::FALSE,
            header: Value::NULL,
            header_0: Value::NULL,
            pages: Value::FALSE,
            systems: Value::FALSE,
            print_elements: Vec::new(),
            print_bookparts: false,
            performances: Vec::new(),
        });
        vm.heap_mut().root(book);
        let pb = PageBreaking::for_pages(&mut vm, book);
        assert_eq!(
            page::make_pages(&mut vm, &pb, &[2, 1], Value::NULL).unwrap(),
            Value::NULL
        );
        let pages = page::make_pages(&mut vm, &pb, &[2, 1], systems)
            .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
        vm.define("%pages", pages);
        // The number each footnote took.
        let table = vm
            .eval_text(
                "(let ((t (ly:output-def-lookup paged 'number-footnote-table)))
                   (map (lambda (k) (ly:stencil-expr (cdr (assq k t)))) '(a b c)))",
                None,
            )
            .unwrap();
        assert_eq!(written(&vm, table), numbers, "{reset}");
        let feet = vm
            .eval_text(
                "(map (lambda (p) (ly:stencil-extent (ly:prob-property p 'foot-stencil) 1)) %pages)",
                None,
            )
            .unwrap();
        // The second line's footnote, 2 high, under the first's by 0.5 on the
        // first page; the third's on the second.
        assert_eq!(written(&vm, feet), "((0.0 . 3.5) (0.0 . 3.0))", "{reset}");
        let tables = vm
            .eval_text(
                "(map (lambda (v) (ly:output-def-lookup paged v))
                      '(label-page-table label-absolute-page-table label-page-string-table absolute-page-counter))",
                None,
            )
            .unwrap();
        assert_eq!(
            written(&vm, tables),
            r#"(((l3 . 2) (l1 . 1) (l2 . 1) (old . 1)) ((l3 . 6) (l1 . 5) (l2 . 5)) ((l3 . "arabic2") (l1 . "arabic1") (l2 . "arabic1")) 6)"#
        );
    }
}

/// A `Paper_book` printing `elements` on `paper`, the part of `parent`'s
/// book, with the header `header`.
fn printing(vm: &mut Vm, paper: Value, parent: Value, header: Value, elements: &[Value]) -> Value {
    let book = vm.heap_mut().wrap(PaperBook {
        paper,
        parent,
        header,
        header_0: Value::NULL,
        pages: Value::FALSE,
        systems: Value::FALSE,
        print_elements: elements.to_vec(),
        print_bookparts: false,
        performances: Vec::new(),
    });
    vm.heap_mut().root(book);
    book
}

/// `Paper_book::pages` (MS3 steps 35 and 62): a book's pages, the paper's
/// `page-breaking`'s, here `ly:optimal-breaking`: the lines of its markup
/// lists in order on the one page they fit; each page's stencil `(lily page)`'s `page-stencil`; the
/// paper's `page-post-process` called with the paper and the pages; the
/// book's systems set from the pages, unless they were made; the pages
/// kept; a book printing its parts their pages, and one printing nothing
/// none. `make-page`, `calc-printable-height` and `page-stencil` are
/// stand-ins, as the image's are run on path.ly's page by the core.
#[test]
fn a_book_s_pages_are_its_page_breaker_s() {
    let mut vm = bare_vm();
    let paper = page_paper(&mut vm, 3);
    vm.eval_text(
        "(define-public (layout-extract-page-properties layout) '())
         (define-public (markup-list? x) (and (pair? x) (eq? (car x) 'lines)))
         (define-public (interpret-markup-list paper props ml)
           (map (lambda (h) (ly:make-stencil (list 'round-filled-box 0 10 0 h 0) '(0 . 10) (cons 0 h)))
                (cdr ml)))
         (define-public %stenciled '())
         (define-public %post '())
         (ly:output-def-set-variable! paged 'page-post-process
           (lambda (paper pages) (set! %post (cons (cons paper pages) %post))))
         (define-module (lily page))
         (define-public (page-stencil page)
           (set! %stenciled (cons page %stenciled))
           (ly:prob-set-property! page 'stencil
             (ly:make-stencil (list 'num (ly:prob-property page 'page-number)) '(0 . 1) '(0 . 1))))
         (set-current-module (resolve-module '(lily)))",
        None,
    )
    .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    let lines = vm.eval_text("'(lines 60 50)", None).unwrap();
    let more = vm.eval_text("'(lines 20)", None).unwrap();
    let book = printing(&mut vm, paper, Value::FALSE, Value::NULL, &[lines, more]);

    let pages =
        paper_book::pages(&mut vm, book).unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    vm.define("%pages", pages);
    let got = vm
        .eval_text(
            "(map (lambda (p)
                    (list (ly:prob-property p 'page-number)
                          (map (lambda (l) (cdr (ly:stencil-extent (ly:prob-property l 'stencil) 1)))
                               (ly:prob-property p 'lines))
                          (ly:prob-property p 'configuration)
                          (ly:stencil-expr (ly:prob-property p 'stencil))))
                  %pages)",
            None,
        )
        .unwrap();
    // One page, the last, of the three lines at the force 0 of no page
    // before: LilyPond 2.26.0's configuration of a `\markuplist` of two
    // boxes 60 and 50 high and a `\markup` of one 20 high on
    // `page_paper`'s paper, which its page breaker puts on one page.
    assert_eq!(
        written(&vm, got),
        "((1 (60.0 50.0 20.0) (61.0 111.0 131.5) (num 1)))"
    );
    // The page drawn is the last made; the others were made for their
    // printable heights (`page_height`), each number once, the last
    // pages' apart: the spacer's of each line, from page 0 (its
    // `static_cast<int> (VPOS)`, -1, after the first page number), then
    // each page after the one before.
    let made = vm.eval_text("(reverse %made)", None).unwrap();
    assert_eq!(
        written(&vm, made),
        "((0 #f) (1 #f) (2 #f) (0 #t) (2 #t) (1 #t) (1 #t))"
    );
    let systems = vm.heap().foreign::<PaperBook>(book).unwrap().systems;
    vm.define("%systems", systems);
    let checks = vm
        .eval_text(
            "(list (equal? %stenciled %pages)
                   (and (= (length %post) 1) (eq? (caar %post) paged) (eq? (cdar %post) %pages))
                   (equal? (ly:prob-property (car %pages) 'lines) %systems))",
            None,
        )
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    assert_eq!(written(&vm, checks), "(#t #t #t)");
    // Kept: neither made nor drawn again.
    assert_eq!(paper_book::pages(&mut vm, book).unwrap(), pages);
    let again = vm
        .eval_text("(list (length %made) (length %stenciled))", None)
        .unwrap();
    assert_eq!(written(&vm, again), "(7 1)");

    // A book printing its parts: their pages in order, each part's own,
    // the systems of a part that had them kept.
    let p1 = printing(&mut vm, paper, Value::FALSE, Value::NULL, &[more]);
    let p2 = printing(&mut vm, paper, Value::FALSE, Value::NULL, &[lines]);
    let kept = vm.eval_text("'(kept)", None).unwrap();
    vm.heap_mut().foreign_mut::<PaperBook>(p2).unwrap().systems = kept;
    let top = printing(&mut vm, paper, Value::FALSE, Value::NULL, &[]);
    paper_book::add_bookpart(vm.heap_mut(), top, p1);
    paper_book::add_bookpart(vm.heap_mut(), top, p2);
    let pages = paper_book::pages(&mut vm, top).unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    vm.define("%pages", pages);
    let got = vm
        .eval_text(
            "(map (lambda (p) (length (ly:prob-property p 'lines))) %pages)",
            None,
        )
        .unwrap();
    assert_eq!(written(&vm, got), "(1 2)");
    let p1_pages = vm.heap().foreign::<PaperBook>(p1).unwrap().pages;
    assert_eq!(vm.heap().car(pages), vm.heap().car(p1_pages));
    assert_eq!(vm.heap().foreign::<PaperBook>(p2).unwrap().systems, kept);
    assert_eq!(
        vm.heap().foreign::<PaperBook>(top).unwrap().systems,
        Value::FALSE
    );
    let made = vm.eval_text("(length %made)", None).unwrap();
    let parts_made = written(&vm, made);

    // A book printing nothing: no pages, no page made.
    let empty = printing(&mut vm, paper, Value::FALSE, Value::NULL, &[]);
    assert_eq!(paper_book::pages(&mut vm, empty).unwrap(), Value::NULL);
    let made = vm.eval_text("(length %made)", None).unwrap();
    assert_eq!(written(&vm, made), parts_made);
}

/// What `page.scm` and `titling.scm` call on a page (MS3 step 35):
/// `ly:make-prob`, `ly:paper-book-paper` and `ly:paper-book-scopes`
/// (`Paper_book::get_scopes`), `ly:modules-lookup`, `(lily)`'s
/// `ice9-format` and the `use-paper-size-for-page` option.
#[test]
fn the_page_s_primitives() {
    let mut vm = bare_vm();
    // The keys set after the immutable ones, each consed first; a last
    // key without a value ignored.
    assert_eq!(
        run(
            &mut vm,
            "(let ((p (ly:make-prob 'page '((a . 1)) 'b 2 'c 3 'd)))
               (list (ly:prob-immutable-properties p)
                     (ly:prob-mutable-properties p) (ly:prob-property p 'a)))"
        ),
        (String::new(), "(((a . 1)) ((c . 3) (b . 2)) 1)".to_string())
    );
    let one = vm.heap_mut().integer(1);
    let two = vm.heap_mut().integer(2);
    let h1 = header(&mut vm, &[("tagline", one)]);
    let h2 = header(&mut vm, &[("tagline", two), ("title", two)]);
    vm.define("%h1", h1);
    vm.define("%h2", h2);
    vm.eval_text("(define-public %in-lily 'lily)", None)
        .unwrap();
    assert_eq!(
        run(
            &mut vm,
            "(list (ly:modules-lookup (list %h1 %h2) 'tagline)
                   (ly:modules-lookup (list %h1 %h2) 'title)
                   (ly:modules-lookup (list %h1) '%in-lily)
                   (ly:modules-lookup (list %h1) 'none)
                   (ly:modules-lookup (list %h1) 'none 'default)
                   (ly:modules-lookup '() 'none))"
        ),
        (String::new(), "(1 2 lily #f default #f)".to_string())
    );
    assert_eq!(
        run(&mut vm, "(ly:modules-lookup (list 1) 'x)").0,
        "wrong-type-arg"
    );

    let paper = vm.eval_text("(ly:make-output-def)", None).unwrap();
    vm.heap_mut().root(paper);
    let book = printing(&mut vm, paper, Value::FALSE, h2, &[]);
    let part = printing(&mut vm, paper, book, h1, &[]);
    let bare = printing(&mut vm, paper, book, Value::FALSE, &[]);
    vm.define("%paper", paper);
    vm.define("%book", book);
    vm.define("%part", part);
    vm.define("%bare", bare);
    assert_eq!(
        run(
            &mut vm,
            "(list (eq? (ly:paper-book-paper %part) %paper)
                   (equal? (ly:paper-book-scopes %part) (list %h1 %h2))
                   (equal? (ly:paper-book-scopes %bare) (list %h2))
                   (equal? (ly:paper-book-scopes %book) (list %h2)))"
        ),
        (String::new(), "(#t #t #t #t)".to_string())
    );
    assert_eq!(run(&mut vm, "(ly:paper-book-paper 1)").0, "wrong-type-arg");
    assert_eq!(run(&mut vm, "(ly:paper-book-scopes 1)").0, "wrong-type-arg");

    assert_eq!(
        run(
            &mut vm,
            "(list (ice9-format #f \"~d\" 12) (ice9-format #f \"~@r\" 12)
                   (ly:get-option 'use-paper-size-for-page))"
        ),
        (String::new(), r#"("12" "XII" #t)"#.to_string())
    );
}

/// A line of `height` hanging below its refpoint, `tallness` below the
/// line before, the spring after it `space` long: a page breaker's
/// `Line_details`, by hand.
fn hand_line(vm: &mut Vm, height: f64, tallness: f64, space: f64) -> LineDetails {
    let mut l = LineDetails::new(vm.heap_mut());
    let iv = Interval::new(-height, 0.0);
    l.shape = LineShape::new(iv, iv);
    l.tallness = tallness;
    l.space = space;
    l.force = 0.0;
    l
}

/// A page breaker of a book on [`page_paper`]'s paper whose current
/// breaks end the book and whose one configuration's lines are `lines`,
/// cached as `cache_line_details` would leave them; `top-system-spacing`'s
/// and `last-bottom-spacing`'s paddings of 1 and no minimum distance.
fn hand_breaker(vm: &mut Vm, lines: Vec<LineDetails>) -> PageBreaking {
    let paper = page_paper(vm, 0);
    let book = printing(vm, paper, Value::FALSE, Value::NULL, &[]);
    let mut pb = PageBreaking::for_pages(vm, book);
    pb.top_system_spacing = (0.0, 1.0);
    pb.last_bottom_spacing = (0.0, 1.0);
    pb.paper_height = 169.00937007874015;
    pb.breaks = vec![BreakPosition::start()];
    pb.current_end_breakpoint = 0;
    pb.current_configurations = vec![vec![lines.len()]];
    pb.cached_configuration_index = 0;
    pb.uncompressed_line_details = lines.clone();
    pb.cached_line_details = lines;
    pb
}

/// `Page_spacing` (MS3 step 62, `page-spacing.cc:28–131`): the force that
/// fills a page of 100 with lines 10 high and 10 tall, springs of 15 after
/// them, by hand: one line (100 − 1 − 1 − 10) / 1, two (98 − 20 − 5) / 2,
/// the same appended or prepended; −∞ for lines that overfill the page; a
/// line's footnotes 3 high with the paddings of 0.5 and the separator before
/// the page's first.
#[test]
fn page_spacing_finds_the_force_that_fills_a_page() {
    let mut vm = bare_vm();
    let a = hand_line(&mut vm, 10.0, 10.0, 15.0);
    let pb = hand_breaker(&mut vm, vec![]);

    let mut down = PageSpacing::new(vm.heap_mut(), 100.0);
    down.append_system(&pb, &a);
    assert_eq!(down.force, 88.0);
    down.append_system(&pb, &a);
    assert_eq!(
        (down.rod_height, down.spring_len, down.force),
        (20.0, 5.0, 36.5)
    );

    let mut up = PageSpacing::new(vm.heap_mut(), 100.0);
    up.prepend_system(&pb, &a);
    assert_eq!(up.force, 88.0);
    up.prepend_system(&pb, &a);
    assert_eq!((up.rod_height, up.spring_len, up.force), (20.0, 5.0, 36.5));

    up.resize(&pb, 20.0);
    assert_eq!(up.force, f64::NEG_INFINITY);
    up.clear();
    assert_eq!((up.rod_height, up.spring_len, up.force), (0.0, 0.0, 0.0));

    let mut pb = pb;
    pb.footnote_padding = 0.5;
    pb.footnote_footer_padding = 0.5;
    pb.footnote_separator_stencil_height = 2.0;
    let mut noted = a.clone();
    noted.footnote_heights = vec![3.0];
    let mut page = PageSpacing::new(vm.heap_mut(), 100.0);
    // The separator, the padding, the note and its padding, the padding
    // taken off the last and the footer's put on: 2 + 0.5 + 3 + 0.5.
    assert_eq!(page.account_for_footnotes(&pb, &noted), 6.0);
    // The separator only once a page.
    assert_eq!(page.account_for_footnotes(&pb, &noted), 3.5);
    page.clear();
    page.append_system(&pb, &noted);
    assert_eq!((page.rod_height, page.force), (16.0, 82.0));
}

/// `Page_spacer` and the page breaker's page side (MS3 step 62,
/// `page-spacing.cc:133–405`, `page-breaking.cc:1185–1755`) on three lines
/// 60 high and tall on [`page_paper`]'s pages, 157.63 printable
/// (`calc-printable-height`'s stand-in), whose last page is ragged: two
/// fit a page, three do not. Their best pages two and one, the first's
/// force (155.63 − 120) / 2 and the ragged last's 0; on three pages one a
/// page; on one page the warning, two lines on it and the third tacked on
/// at [`BAD_SPACING_PENALTY`]; at least two pages; on two pages, the
/// first line alone after a forced page break; the demerits the lines'
/// and the pages' forces' squares, these weighted by 10 (no
/// `page-spacing-weight`); a fixed number a page, and packed; the line
/// counts' penalties.
#[test]
fn the_page_spacer_puts_lines_on_pages() {
    let mut vm = bare_vm();
    let a = hand_line(&mut vm, 60.0, 60.0, 15.0);
    let mut pb = hand_breaker(&mut vm, vec![a.clone(), a.clone(), a.clone()]);
    let lines = pb.cached_line_details.clone();
    let first = (169.00937007874015 - 2.0 * 5.690551181102362 - 2.0 - 120.0) / 2.0;

    let mut ps = PageSpacer::new(&lines, 1, &pb);
    let best = ps
        .solve_best(&mut vm, &mut pb)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    assert_eq!(best.systems_per_page, [2, 1]);
    assert_eq!(best.force, [first, 0.0]);
    // Left for finalize_spacing_result, as LilyPond's.
    assert_eq!(best.demerits, f64::INFINITY);

    let three = ps.solve(&mut vm, &mut pb, 3).unwrap();
    assert_eq!(three.systems_per_page, [1, 1, 1]);
    let warnings = vm.eval_text("%warnings", None).unwrap();
    assert_eq!(written(&vm, warnings), "()");
    let one = ps.solve(&mut vm, &mut pb, 1).unwrap();
    assert_eq!(
        (one.systems_per_page.clone(), one.force.clone()),
        (vec![3], vec![BAD_SPACING_PENALTY])
    );
    let warnings = vm.eval_text("%warnings", None).unwrap();
    assert_eq!(
        written(&vm, warnings),
        "(\"tried to space systems on a bad number of pages\")"
    );

    assert_eq!(pb.min_page_count(&mut vm, 0, 1).unwrap(), 2);

    let two = pb.space_systems_on_2_pages(&mut vm, 0, 1).unwrap();
    assert_eq!(
        (two.systems_per_page.clone(), two.force.clone()),
        (vec![2, 1], vec![first, 0.0])
    );
    let force = vm.heap_mut().intern("force");
    pb.cached_line_details[0].page_permission = force;
    let forced = pb.space_systems_on_2_pages(&mut vm, 0, 1).unwrap();
    assert_eq!(forced.systems_per_page, [1, 2]);
    pb.cached_line_details[0].page_permission = a.page_permission;

    let mut res = best.clone();
    pb.uncompressed_line_details[1].force = 2.0;
    pb.uncompressed_line_details[2].break_penalty = 5.0;
    res.penalty = 1.0;
    let done = pb.finalize_spacing_result(&mut vm, 0, res).unwrap();
    // The ragged last page's force is not counted.
    assert_eq!(done.demerits, 4.0 + 5.0 + (1.0 + first * first) * 10.0);
    assert_eq!(done.systems_per_page, [2, 1]);

    let on_best = pb.space_systems_on_best_pages(&mut vm, 0, 1).unwrap();
    assert_eq!(on_best.systems_per_page, [2, 1]);
    let packed = pb.pack_systems_on_least_pages(&mut vm, 0, 1).unwrap();
    assert_eq!(packed.systems_per_page, [2, 1]);

    pb.systems_per_page = 1;
    let fixed = pb
        .space_systems_with_fixed_number_per_page(&mut vm, 0, 1)
        .unwrap();
    assert_eq!(
        (fixed.systems_per_page.clone(), fixed.system_count_status),
        (vec![1, 1, 1], SYSTEM_COUNT_OK)
    );
    assert_eq!(pb.line_count_penalty(3), 2.0 * TERRIBLE_SPACING_PENALTY);
    assert_eq!(pb.line_count_status(3), SYSTEM_COUNT_TOO_MANY);
    assert_eq!(pb.line_count_status(0), SYSTEM_COUNT_TOO_FEW);
    assert!(pb.all_lines_stretched(&mut vm, 0).unwrap());
}

/// `Line_details (pb, paper)` of a title's paper system (MS3 step 62,
/// `constrained-breaking.cc:576–638`): no force, its shape its stencil's
/// extent all the way across, the paper's `markup-markup-spacing` before
/// a title (`markup-system-spacing` none on [`page_paper`]'s paper), a
/// title's no non-title line, its permissions the prob's.
#[test]
fn a_title_s_line_details_are_its_stencil_s_and_the_paper_s() {
    let mut vm = bare_vm();
    let paper = page_paper(&mut vm, 0);
    let line = markup_line(&mut vm, -20.0, 5.0);
    vm.eval_text(
        "(ly:prob-set-property! %line 'page-break-permission 'force)
         (ly:prob-set-property! %line 'page-break-penalty 3)",
        None,
    )
    .unwrap();
    let d = LineDetails::of_prob(&mut vm, line, paper)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    assert_eq!(d.force, 0.0);
    assert_eq!(
        d.shape,
        LineShape::new(Interval::new(-20.0, 5.0), Interval::new(-20.0, 5.0))
    );
    assert_eq!(d.full_height(), 25.0);
    assert_eq!(
        (d.title_space, d.title_padding, d.space, d.padding),
        (1.0, 0.5, 0.0, 0.0)
    );
    assert_eq!((d.title, d.compressed_nontitle_lines_count), (true, 0));
    assert_eq!(d.page_penalty, 3.0);
    let force = vm.heap_mut().intern("force");
    assert_eq!(d.page_permission, force);
}

/// paper-book-scheme.cc's and page-breaking-scheme.cc's face (MS3 step
/// 62): `ly:paper-book-header`, `ly:paper-book-pages` and
/// `ly:paper-book-systems` of a `Paper_book`, and `ly:optimal-breaking`'s
/// and their type errors for what is none; since MS8 step 9's part 3 the
/// other five breakers' type errors too.
#[test]
fn the_paper_book_s_and_the_page_breaker_s_primitives() {
    let mut vm = bare_vm();
    let paper = page_paper(&mut vm, 1);
    let one = vm.heap_mut().integer(1);
    let h = header(&mut vm, &[("title", one)]);
    let book = printing(&mut vm, paper, Value::FALSE, h, &[]);
    vm.define("%book", book);
    vm.define("%h", h);
    vm.eval_text(
        "(define-public (layout-extract-page-properties layout) '())",
        None,
    )
    .unwrap();
    assert_eq!(
        run(
            &mut vm,
            "(list (eq? (ly:paper-book-header %book) %h)
                   (ly:paper-book-pages %book) (ly:paper-book-systems %book))"
        ),
        (String::new(), "(#t () ())".to_string())
    );
    for call in [
        "(ly:paper-book-header 1)",
        "(ly:paper-book-pages 1)",
        "(ly:paper-book-systems 1)",
        "(ly:optimal-breaking 1)",
        "(ly:page-turn-breaking 1)",
        "(ly:minimal-breaking 1)",
        "(ly:one-page-breaking 1)",
        "(ly:one-line-breaking 1)",
        "(ly:one-line-auto-height-breaking 1)",
    ] {
        assert_eq!(run(&mut vm, call).0, "wrong-type-arg", "{call}");
    }
}
