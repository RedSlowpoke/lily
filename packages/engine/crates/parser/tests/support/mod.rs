//! What the parser's tests, the runner (`examples/parse.rs`) and the spike's
//! image writer (`crates/core/examples/bench.rs`) share: the installed
//! LilyPond 2.26.0's `scm/lily/`, and a `(lily)` loaded from it and from
//! `ly/music-functions-init.ly` as the parser needs it, which the image
//! holds (step 19), with stubs of the parts on demand (D74), which load
//! from it when called.

use lily_engine_parser::NoteNames;
use lily_engine_scheme::{Heap, Hosted, LilyScan, Value, Vm, read};
use std::path::{Path, PathBuf};
use std::process::Command;

/// `scm/lily/` of the installed LilyPond 2.26.0, if there is one.
pub fn lily_scm() -> Option<PathBuf> {
    let version = Command::new("lilypond").arg("--version").output().ok()?;
    if !String::from_utf8_lossy(&version.stdout).contains("GNU LilyPond 2.26.0") {
        return None;
    }
    let path = Command::new("sh")
        .args(["-c", "command -v lilypond"])
        .output()
        .ok()?;
    let binary = std::fs::canonicalize(String::from_utf8(path.stdout).ok()?.trim()).ok()?;
    let lily = binary.parent()?.join("../share/lilypond/2.26.0/scm/lily");
    lily.join("music-functions.scm").exists().then_some(lily)
}

fn text(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The form of `text` that begins at `start`, as its text.
fn form_at(text: &str, start: usize, what: &str) -> Result<String, String> {
    let mut heap = Heap::new();
    let r = read(&mut heap, text, start, None).map_err(|e| format!("{what}: {e:?}"))?;
    Ok(text[r.start..r.end].to_string())
}

/// The toplevel form of `file` that begins with `head`, as its text.
fn definition(lily: &Path, file: &str, head: &str) -> Result<String, String> {
    let text = text(&lily.join(file))?;
    let start = text
        .find(head)
        .ok_or_else(|| format!("{file} has no {head}"))?;
    form_at(&text, start, file)
}

/// The definitions of `lily.scm` that the library files, music functions'
/// signatures and `titling-init.ly` call, and its deprecated properties, which
/// `define-context-properties.scm` defines with `define-deprecated-property`:
/// LilyPond has loaded them before the files.
const DEFINITIONS: [(&str, &str); 11] = [
    ("lily.scm", "(define-public (index? x)"),
    ("lily.scm", "(define-public (lilypond-version)"),
    ("lily.scm", "(define-public (positive-number? x)"),
    ("lily.scm", "(define (positive-exact-integer? x)"),
    (
        "lily.scm",
        "(define deprecated-translation-getter-description",
    ),
    (
        "lily.scm",
        "(define deprecated-translation-setter-description",
    ),
    ("lily.scm", "(define deprecated-setter-object-property"),
    ("lily.scm", "(set! (deprecated-setter-object-property"),
    ("lily.scm", "(define deprecated-getter-object-property"),
    ("lily.scm", "(set! (deprecated-getter-object-property"),
    ("lily.scm", "(define*-public (define-deprecated-property"),
];

/// The constructors of `ly-syntax-constructors.scm` that a function cut
/// short by `\etc` is made with and later called through (P11, MS2 step
/// 34, D93), defined after [`FILES`], whose `define-syntax-function` they
/// use: `partial-music-function`, the `music-function` and
/// `music-function-chain` it calls, the syntax functions of the calls it
/// chains (`property-set`, `property-override`, `repeat`,
/// `create-script-function`), `partial-text-script` and `partial-markup`;
/// and `composed-markup-list`, which markup's heads are composed with (P7).
/// The parser's own calls of constructors are the music crate's (D69).
/// Since MS8 step 10 (OPEN F) they are defined in their own module,
/// `(lily ly-syntax-constructors)`, as LilyPond's, which `(lily)` does not
/// export; the parser calls them there ([`CONSTRUCTOR_MODULE`]).
const CONSTRUCTORS: [(&str, &str); 11] = [
    (
        "ly-syntax-constructors.scm",
        "(define-public (composed-markup-list",
    ),
    (
        "ly-syntax-constructors.scm",
        "(define-public (music-function fun args)",
    ),
    (
        "ly-syntax-constructors.scm",
        "(define (music-function-chain call final)",
    ),
    (
        "ly-syntax-constructors.scm",
        "(define-public (partial-music-function call-list)",
    ),
    (
        "ly-syntax-constructors.scm",
        "(define-public (partial-text-script partial-markup)",
    ),
    (
        "ly-syntax-constructors.scm",
        "(define-public (create-script item)",
    ),
    (
        "ly-syntax-constructors.scm",
        "(define-public create-script-function",
    ),
    ("ly-syntax-constructors.scm", "(define-public repeat"),
    ("ly-syntax-constructors.scm", "(define-public property-set"),
    (
        "ly-syntax-constructors.scm",
        "(define-public property-override",
    ),
    (
        "ly-syntax-constructors.scm",
        "(define-public (partial-markup commands)",
    ),
];

/// The Scheme files loaded whole into `(lily)`, in `lily.scm`'s order: its
/// library files, and 03 §6's init data among the core files of its body
/// (MS2 step 12, D75). `color.scm` gives the colors that property values
/// name (`#red`), `define-music-callbacks.scm` procedures the music types
/// name, `c++.scm` the type predicates that signatures name, and
/// `define-grobs.scm` the grobs' names that grob paths begin with, and
/// `midi.scm` the performances' writer, `write-performances-midis`, and
/// what MIDI's performers call (MS5 step 1), and `scheme-performers.scm`
/// `Beat_performer` (step 5).
const FILES: [&str; 38] = [
    "color.scm",
    "lily-library.scm",
    "output-lib.scm",
    "markup-macros.scm",
    "parser-ly-from-scheme.scm",
    "file-cache.scm",
    "define-event-classes.scm",
    "define-music-callbacks.scm",
    "define-note-names.scm",
    "c++.scm",
    "skyline.scm",
    "markup.scm",
    "define-markup-commands.scm",
    "stencil.scm",
    "music-functions.scm",
    "define-music-properties.scm",
    "time-signature.scm",
    "time-signature-settings.scm",
    "auto-beam.scm",
    "define-context-properties.scm",
    "translation-functions.scm",
    "breath.scm",
    "script.scm",
    "midi.scm",
    "layout-beam.scm",
    "parser-clef.scm",
    "layout-slur.scm",
    "bar-line.scm",
    "flag-styles.scm",
    "define-grob-properties.scm",
    "define-grobs.scm",
    "define-grob-interfaces.scm",
    "define-stencil-commands.scm",
    "scheme-engravers.scm",
    "scheme-performers.scm",
    "titling.scm",
    "paper.scm",
    "context.scm",
];

/// The modules of `scm/lily/` that [`FILES`] use, which are in the image:
/// `titling.scm` uses `(lily page)`, which uses `(lily paper-system)`; and
/// `(lily)` uses `(lily paper-system)`, as `backend-library.scm:22–23`
/// makes it, for `define-markup-commands.scm`'s `score-lines` (MS3 step
/// 43).
const MODULES: [(&str, &str); 2] = [
    ("lily paper-system", "paper-system.scm"),
    ("lily page", "page.scm"),
];

/// The modules of their own a part's data holds besides its files, each
/// with its part and its file: a `use-modules` of it loads the file
/// (`%load-module-source`, D273), not the part. `(lily accreg)` is
/// `diagrams`' (03 §6), whose `define-register-set` is a `syntax-case`
/// macro.
pub(crate) const MODULE_FILES: [(&str, &str, &str); 1] =
    [("diagrams", "lily accreg", "accreg.scm")];

/// 03 §6's parts on demand (D74), each with its files in `lily.scm`'s
/// order: not in the image, but loaded from source when one of their
/// stubs is called. `qr`, which `define-markup-commands.scm` imports
/// `qr-encode` of, is one too. `accreg.scm` of `diagrams` is a module of
/// its own, which a score uses by name, and is not among them
/// ([`MODULE_FILES`]).
pub(crate) const PARTS: [(&str, &[&str]); 7] = [
    (
        "chords",
        &[
            "chord-entry.scm",
            "chord-ignatzek-names.scm",
            "chord-name.scm",
        ],
    ),
    (
        "display",
        &["display-lily.scm", "define-music-display-methods.scm"],
    ),
    (
        "fretted",
        &[
            "fret-diagrams.scm",
            "tablature.scm",
            "predefined-fretboards.scm",
        ],
    ),
    (
        "diagrams",
        &[
            "harp-pedals.scm",
            "define-woodwind-diagrams.scm",
            "display-woodwind-diagrams.scm",
        ],
    ),
    ("partcombine", &["part-combiner.scm", "autochange.scm"]),
    ("modal", &["modal-transforms.scm"]),
    ("qr", &["qr-code.scm"]),
];

/// The names `text`, a Scheme file, defines at its toplevel for other
/// files: by `define-public`, `define*-public` and `define-session-public`,
/// and a markup command's two, `name-markup` and `make-name-markup` (with
/// `-list` after a list command's); and the module its `define-module`
/// names, if it has one.
fn public_names(text: &str) -> Result<(Vec<String>, Option<String>), String> {
    let mut heap = Heap::new();
    let mut names = Vec::new();
    let mut module = None;
    let mut at = 0;
    loop {
        let mut scan = LilyScan { file: None };
        let r = read(&mut heap, text, at, Some(&mut scan)).map_err(|e| format!("{e:?}"))?;
        if r.datum == Value::EOF {
            return Ok((names, module));
        }
        at = r.end;
        let (Some(head), Some(rest)) = (heap.car(r.datum), heap.cdr(r.datum)) else {
            continue;
        };
        let Some(mut name) = heap.car(rest) else {
            continue;
        };
        // A curried head's name is its innermost car.
        while let Some(car) = heap.car(name) {
            name = car;
        }
        let name = heap.name(name).unwrap_or_default().to_string();
        match heap.name(head).unwrap_or_default() {
            "define-public" | "define*-public" | "define-session-public" => names.push(name),
            "define-markup-command" => {
                names.push(format!("{name}-markup"));
                names.push(format!("make-{name}-markup"));
            }
            "define-markup-list-command" => {
                names.push(format!("{name}-markup-list"));
                names.push(format!("make-{name}-markup-list"));
            }
            "define-module" => {
                let mut path = Vec::new();
                let mut p = heap.car(rest).unwrap_or(Value::NULL);
                while let (Some(car), Some(cdr)) = (heap.car(p), heap.cdr(p)) {
                    path.push(heap.name(car).unwrap_or_default().to_string());
                    p = cdr;
                }
                module = Some(path.join(" "));
            }
            _ => {}
        }
    }
}

/// [`PARTS`] in `(lily)`'s `%parts`, and each name a part defines bound
/// to its stub in the part's module: `(lily)`, or the module its first file
/// names, which is made here. The table is read from the installed files,
/// as D74 has the build generate it.
fn part_stubs(vm: &mut Vm, lily: &Path) -> Result<(), String> {
    let mut table = String::from("(set! %parts '(");
    for (part, files) in PARTS {
        table.push_str(&format!("({part}"));
        for file in files.iter() {
            table.push_str(&format!(" {file:?}"));
        }
        table.push(')');
    }
    table.push_str("))\n");
    let mut stubs = String::new();
    for (part, _) in PARTS {
        stubs.push_str(&stubs_of(lily, part)?);
    }
    vm.eval_text(&(table + &stubs), None)
        .map_err(|e| e.text(vm.heap()))?;
    Ok(())
}

/// The definitions that bind each name `part` defines to its stub, in the
/// part's module, and leave `(lily)` current.
fn stubs_of(lily: &Path, part: &str) -> Result<String, String> {
    let files = PARTS
        .iter()
        .find(|(p, _)| *p == part)
        .ok_or_else(|| format!("no part {part}"))?
        .1;
    let mut stubs = String::new();
    let mut module = None;
    for file in files {
        let (names, named) =
            public_names(&text(&lily.join(file))?).map_err(|e| format!("{file}: {e}"))?;
        let module = module.get_or_insert_with(|| {
            let module = named.unwrap_or_else(|| "lily".into());
            if module != "lily" {
                stubs.push_str(&format!("(define-module ({module}))\n"));
            }
            stubs.push_str(&format!(
                "(set-current-module (resolve-module '({module})))\n"
            ));
            module
        });
        for name in names {
            stubs.push_str(&format!(
                "(define-public {name} ((@@ (lily) %part-stub) '{part} '({module}) '{name}))\n"
            ));
        }
    }
    stubs.push_str("(set-current-module (resolve-module '(lily)))\n");
    Ok(stubs)
}

/// The hooks by which a part's own module loads the part when a file uses
/// it, as Guile loads the module's file: its macros (`display`'s
/// `with-music-match`) have no stubs. Made after the init, since the core
/// uses `qr`'s module at load.
fn part_module_hooks(lily: &Path) -> Result<String, String> {
    let mut hooks = String::new();
    for (part, files) in PARTS {
        let (_, module) =
            public_names(&text(&lily.join(files[0]))?).map_err(|e| format!("{}: {e}", files[0]))?;
        if let Some(module) = module.filter(|m| m != "lily") {
            hooks.push_str(&format!(
                "(set! %module-hooks (acons '({module}) (lambda () (%load-part! '{part})) %module-hooks))\n"
            ));
        }
    }
    for (_, module, file) in MODULE_FILES {
        hooks.push_str(&format!(
            "(set! %module-hooks (acons '({module}) (lambda () (%load-module-source {file:?})) %module-hooks))\n"
        ));
    }
    Ok(hooks)
}

/// The parts `declarations-init.ly` reads values of at load (D78):
/// `chord-modifiers-init.ly`, `string-tunings-init.ly` and
/// `engraver-init.ly` read or call what they define. The image writer
/// loads them before the init and binds their public names to their stubs
/// again after it, so the image keeps the values the init stored, and of
/// the parts only what those reach.
const SNAPSHOT: [&str; 2] = ["chords", "fretted"];

/// Where `define-music-types.scm` comes among [`FILES`]: the music types'
/// table is made before the file at this index (N2).
const TYPES_AFTER: usize = 8;

/// Where `music-functions.scm` comes among [`FILES`]: LilyPond loads
/// `(lily display-lily)` with it, whose display methods every music type
/// then has; the image has their stubs from there on (OPEN C6).
const DISPLAY_AFTER: usize = 15;

/// Each music type's display methods as `define-music-display-methods.scm`
/// defines them, in its order, as `%display-method-stubs!` takes them:
/// `define-display-method` (and the post-event and span-event forms that
/// expand to it) makes the type's one method, named `method` after its
/// `let`; `define-extra-display-method` puts a `new-method` before them.
fn display_methods(lily: &Path) -> Result<String, String> {
    let text = text(&lily.join("define-music-display-methods.scm"))?;
    let mut heap = Heap::new();
    let mut table: Vec<(String, Vec<(&str, String)>)> = Vec::new();
    fn walk(heap: &Heap, v: Value, table: &mut Vec<(String, Vec<(&'static str, String)>)>) {
        let (Some(head), Some(rest)) = (heap.car(v), heap.cdr(v)) else {
            return;
        };
        let kind = match heap.name(head) {
            Some(
                "define-display-method"
                | "define-post-event-display-method"
                | "define-span-event-display-method",
            ) => Some("method"),
            Some("define-extra-display-method") => Some("new-method"),
            Some("define-syntax-rule") => return,
            _ => None,
        };
        let ty = heap.car(rest).and_then(|t| heap.name(t));
        let param = heap
            .cdr(rest)
            .and_then(|r| heap.car(r))
            .and_then(|f| heap.car(f))
            .and_then(|p| heap.name(p));
        if let (Some(kind), Some(ty), Some(param)) = (kind, ty, param) {
            let at = match table.iter().position(|(t, _)| t == ty) {
                Some(at) => at,
                None => {
                    table.push((ty.into(), Vec::new()));
                    table.len() - 1
                }
            };
            let methods = &mut table[at].1;
            match kind {
                "method" => *methods = vec![(kind, param.into())],
                _ => methods.insert(0, (kind, param.into())),
            }
        }
        let mut at = v;
        while let (Some(item), Some(next)) = (heap.car(at), heap.cdr(at)) {
            walk(heap, item, table);
            at = next;
        }
    }
    let mut at = 0;
    loop {
        let r = read(&mut heap, &text, at, None).map_err(|e| format!("{e:?}"))?;
        if r.datum == Value::EOF {
            break;
        }
        at = r.end;
        walk(&heap, r.datum, &mut table);
    }
    let mut out = String::from("(%display-method-stubs! '(");
    for (ty, methods) in &table {
        out.push_str(&format!("({ty}"));
        for (name, param) in methods {
            out.push_str(&format!(" ({name} . {param})"));
        }
        out.push(')');
    }
    out.push_str("))\n");
    Ok(out)
}

/// Loads into `vm`'s `(lily)`, which has the music model's `ly:` functions,
/// what the parser needs of `lily`, LilyPond's `scm/lily/`: [`MODULES`];
/// [`DEFINITIONS`]; the stubs of [`PARTS`]; [`FILES`], with the music
/// types' table that `make-music` needs among them
/// (`lily_engine_music::define_music_types`); `(lily paper-system)`'s
/// exports; [`CONSTRUCTORS`];
/// `type-p-name-alist` of the
/// predicates there are then;
/// `ly:parser-include-string`;
/// and `ly/declarations-init.ly`, with the files it includes, parsed and
/// run as an init file (`lily_engine_parser::parse_init`), whose variables
/// each file's module then uses, with [`SNAPSHOT`]'s parts loaded while it
/// runs.
pub fn load_library(vm: &mut Vm, lily: &Path) -> Result<(), String> {
    // LilyPond's own Scheme, which Guile would have compiled: its
    // procedures print their formals (OPEN C6).
    vm.eval_text("((@@ (lily) %named-formals) 'toplevel)", None)
        .map_err(|e| e.text(vm.heap()))?;
    let loaded = load_library_named(vm, lily);
    vm.eval_text("((@@ (lily) %named-formals) #f)", None)
        .map_err(|e| e.text(vm.heap()))?;
    loaded
}

fn load_library_named(vm: &mut Vm, lily: &Path) -> Result<(), String> {
    for (name, file) in MODULES {
        vm.provide_module(name, text(&lily.join(file))?);
    }
    let mut prelude = String::from("(set-current-module (resolve-module '(lily)))\n");
    for (file, head) in DEFINITIONS {
        prelude.push_str(&definition(lily, file, head)?);
        prelude.push('\n');
    }
    vm.eval_text(&prelude, None)
        .map_err(|e| e.text(vm.heap()))?;
    part_stubs(vm, lily)?;
    for (n, file) in FILES.into_iter().enumerate() {
        if n == TYPES_AFTER {
            lily_engine_music::define_music_types(vm)?;
        }
        if n == DISPLAY_AFTER {
            let stubs = display_methods(lily)?;
            vm.eval_text(&stubs, None)
                .map_err(|e| format!("the display methods' stubs: {}", e.text(vm.heap())))?;
        }
        let path = lily.join(file);
        if let Err(e) = vm.eval_text(&text(&path)?, path.to_str()) {
            return Err(format!("{file} does not load: {}", e.text(vm.heap())));
        }
    }
    vm.eval_text(
        "(set-current-module (resolve-module '(lily)))\n(use-modules (lily paper-system))",
        None,
    )
    .map_err(|e| e.text(vm.heap()))?;
    let mut constructors = String::from(
        "(define-module (lily ly-syntax-constructors) #:use-module (lily) #:use-module (srfi srfi-1))\n",
    );
    for (file, head) in CONSTRUCTORS {
        constructors.push_str(&definition(lily, file, head)?);
        constructors.push('\n');
    }
    constructors.push_str("(set-current-module (resolve-module '(lily)))\n");
    vm.eval_text(&constructors, None)
        .map_err(|e| e.text(vm.heap()))?;
    lily_engine_parser::define_type_names(vm, &text(&lily.join("lily.scm"))?)?;
    lily_engine_parser::define_include_string(vm)?;
    let names = std::fs::read(lily.join("define-note-names.scm"))
        .map_err(|e| e.to_string())
        .and_then(|t| NoteNames::read(&t))?;
    // The stubs of the parts loaded for the init, which the names are
    // bound to again after it, so that what stored a stub keeps it.
    let parts = SNAPSHOT.join(" ");
    let stubs = vm
        .eval_text(
            &format!(
                "(hash-fold (lambda (stub entry stubs)
                              (if (memq (car entry) '({parts})) (cons (cons stub entry) stubs) stubs))
                            '() (@@ (lily) %stubs))"
            ),
            None,
        )
        .map_err(|e| e.text(vm.heap()))?;
    let depth = vm.heap().root_depth();
    vm.heap_mut().root(stubs);
    vm.set_part_source(Some(part_source(lily)));
    let load = SNAPSHOT
        .iter()
        .map(|part| format!("((@@ (lily) %load-part!) '{part})\n"))
        .collect::<String>();
    vm.eval_text(&load, None).map_err(|e| e.text(vm.heap()))?;
    // The `ly/` files' Scheme is LilyPond's evaluator's, as a score's.
    vm.eval_text("((@@ (lily) %named-formals) #f)", None)
        .map_err(|e| e.text(vm.heap()))?;
    let ly = lily.join("../../ly");
    let init = std::fs::read(ly.join(INIT)).map_err(|e| format!("{INIT}: {e}"))?;
    let parse = lily_engine_parser::parse_init(INIT, init, &names, vm, &mut |name| {
        std::fs::read(ly.join(name)).map_or(Hosted::Missing, Hosted::Text)
    });
    vm.set_part_source(None);
    if parse.failed() {
        return Err(String::from_utf8_lossy(&parse.stderr()).into_owned());
    }
    // The page's text in LilyPond's own families (B9, D210): under the svg
    // backend paper-defaults-init.ly names fontconfig's serif, sans and
    // monospace; the engine's `$defaultpaper` names D106's, each set as
    // capture.ly's `\paper` sets it (`nested_property_alist`).
    let paper = vm
        .module_value(parse.module, "$defaultpaper")
        .ok_or("declarations-init.ly defines no $defaultpaper")?;
    vm.eval_text(TEXT_FAMILIES, None)
        .and_then(|set| vm.apply(set, &[paper]))
        .map_err(|e| e.text(vm.heap()))?;
    let restub = vm
        .eval_text(
            &format!(
                "(lambda (stubs)
               (for-each (lambda (s) (module-define! (resolve-module (caddr s)) (cadddr s) (car s)))
                         stubs)
               (module-set! (resolve-module '(lily)) '%parts-loaded '())
               (module-set! (resolve-module '(lily)) '%parts-kept '({parts}))
               (module-set! (resolve-module '(lily)) '%files-loaded '()))"
            ),
            None,
        )
        .and_then(|restub| vm.apply(restub, &[stubs]))
        .map_err(|e| e.text(vm.heap()));
    vm.heap_mut().unroot_to(depth);
    restub?;
    let hooks = format!(
        "(set-current-module (resolve-module '(lily)))\n{}",
        part_module_hooks(lily)?
    );
    vm.eval_text(&hooks, None).map_err(|e| e.text(vm.heap()))?;
    vm.take_output();
    Ok(())
}

/// The init file [`load_library`] parses, as its locations name it.
pub const INIT: &str = "declarations-init.ly";

/// D106's text families in a paper's `property-defaults`, in capture.ly's
/// order: each entry in front of `fonts`, and `fonts` in front of the rest.
const TEXT_FAMILIES: &str = "(lambda (paper)
  (for-each
    (lambda (family)
      (let* ((defaults (ly:output-def-lookup paper 'property-defaults))
             (fonts (assq 'fonts defaults)))
        (ly:output-def-set-variable! paper 'property-defaults
          (cons (cons 'fonts (cons family (if fonts (cdr fonts) '())))
                (if fonts (delq fonts defaults) defaults)))))
    '((serif . \"LilyPond Serif\") (sans . \"LilyPond Sans Serif\")
      (typewriter . \"LilyPond Monospace\"))))";

/// A VM with the `ly:` functions of the music model and of layout and
/// [`load_library`]'s `(lily)`, which loads a part's files from `lily` when
/// one of its stubs is called.
pub fn library(lily: &Path) -> Result<Vm, String> {
    library_with(lily, |vm| {
        lily_engine_music::install(vm)?;
        lily_engine_layout::install(vm)
    })
}

/// [`library`] with the `ly:` functions `install` defines: the core's
/// (`lily_engine_core::install`), whose images have the notation crates'
/// too (D223).
pub fn library_with(
    lily: &Path,
    install: fn(&mut Vm) -> Result<(), lily_engine_scheme::Exception>,
) -> Result<Vm, String> {
    let mut vm = Vm::new();
    install(&mut vm).map_err(|e| e.text(vm.heap()))?;
    load_library(&mut vm, lily)?;
    vm.set_part_source(Some(part_source(lily)));
    Ok(vm)
}

/// The parts' files read from `lily` (D74, until S6): a name of [`PARTS`]'
/// files only.
pub fn part_source(lily: &Path) -> lily_engine_scheme::PartSource {
    let lily = lily.to_path_buf();
    Box::new(move |name| {
        (PARTS.iter().any(|(_, files)| files.contains(&name))
            || MODULE_FILES.iter().any(|&(_, _, file)| file == name))
        .then(|| std::fs::read_to_string(lily.join(name)).ok())
        .flatten()
        .map_or(Hosted::Missing, Hosted::Text)
    })
}

/// The music of `parse`'s scores in `tools/compare/music.ly`'s format, as
/// LilyPond's book handler gets it after `scorify-music`: those of each
/// `\book` block first, as `toplevel-book-handler` gets them while the
/// file is parsed, each book's scores and then its parts', in the order
/// they were added; then the scores the toplevel handlers collected in the
/// file's `toplevel-scores`, last first, as `ly:book-scores` gives
/// `music.ly` those of the file's book.
// Only `tests/music.rs` and the parse example write trees.
#[allow(dead_code)]
pub fn music_trees(vm: &mut Vm, parse: &lily_engine_parser::Parse) -> Result<String, String> {
    let depth = vm.heap().root_depth();
    vm.heap_mut().root(parse.module);
    let result = (|| {
        let mut out = String::new();
        let mut scores = Vec::new();
        for book in parse.toplevel_books(vm) {
            book_scores(vm, book, &mut scores);
        }
        // `toplevel_scores` gives them in the file's order.
        scores.extend(parse.toplevel_scores(vm).into_iter().rev());
        let mut n = 0;
        for &score in &scores {
            // A book's text is no score, and `music.ly` counts none.
            let Some(music) = vm
                .heap()
                .foreign::<lily_engine_music::Score>(score)
                .filter(|s| !s.error)
                .map(|s| s.music)
            else {
                continue;
            };
            n += 1;
            out.push_str(&format!("score {n}\n"));
            let mut dump = Dump {
                vm: &mut *vm,
                sources: &parse.sources,
                out: &mut out,
                seen: Vec::new(),
            };
            dump.node(music, 0)?;
        }
        Ok(out)
    })();
    vm.heap_mut().unroot_to(depth);
    result
}

/// The scores of `book` and of its parts as `music.ly`'s `music-book!`
/// walks them: `ly:book-scores` and `ly:book-book-parts` as they are kept,
/// the last added first (MS8 step 10: syntax.ly's two `\bookpart`s).
#[allow(dead_code)]
fn book_scores(vm: &Vm, book: Value, out: &mut Vec<Value>) {
    let Some(b) = vm.heap().foreign::<lily_engine_music::Book>(book) else {
        return;
    };
    let (scores, parts) = (b.scores, b.parts);
    let items = |mut list: Value| {
        let mut items = Vec::new();
        while let (Some(item), Some(rest)) = (vm.heap().car(list), vm.heap().cdr(list)) {
            items.push(item);
            list = rest;
        }
        items
    };
    out.extend(items(scores));
    for part in items(parts) {
        book_scores(vm, part, out);
    }
}

/// `music.ly`'s walk.
#[allow(dead_code)]
struct Dump<'a> {
    vm: &'a mut Vm,
    sources: &'a lily_engine_parser::Sources,
    out: &'a mut String,
    seen: Vec<Value>,
}

#[allow(dead_code)]
impl Dump<'_> {
    fn line(&mut self, depth: usize, text: &str) {
        self.out.push_str(&"  ".repeat(depth));
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// `@file:line:column-line:column` of an origin, the file's base name.
    /// Music the init made is placed in the init file named, read from the
    /// installed LilyPond's `ly/`.
    fn origin(&self, v: Value) -> String {
        let Some(input) = self.vm.heap().foreign::<lily_engine_music::Input>(v) else {
            return String::from("@?");
        };
        let init;
        let src = match self.sources.source(input.file) {
            Some(src) if src.name() == &*input.name => src,
            _ => {
                let Some(text) = lily_scm()
                    .and_then(|lily| std::fs::read(lily.join("../../ly").join(&*input.name)).ok())
                else {
                    return String::from("@?");
                };
                init = lily_engine_parser::Source::new(&input.name, text);
                &init
            }
        };
        let name = Path::new(src.name()).file_name().map_or_else(
            || src.name().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let (a, b) = (src.counts(input.start), src.counts(input.end));
        format!("@{name}:{}:{}-{}:{}", a.line, a.column, b.line, b.column)
    }

    fn property(&mut self, music: Value, key: &str) -> Value {
        let key = self.vm.heap_mut().intern(key);
        lily_engine_music::music_property(self.vm.heap_mut(), music, key).unwrap_or(Value::NULL)
    }

    fn is_music(&self, v: Value) -> bool {
        self.vm
            .heap()
            .foreign::<lily_engine_music::Music>(v)
            .is_some()
    }

    /// The items of `v` if it is a proper list, none of them excepted.
    fn list(&self, v: Value) -> Option<Vec<Value>> {
        let heap = self.vm.heap();
        let mut items = Vec::new();
        let mut p = v;
        while let (Some(car), Some(cdr)) = (heap.car(p), heap.cdr(p)) {
            items.push(car);
            p = cdr;
        }
        (p == Value::NULL).then_some(items)
    }

    fn node(&mut self, music: Value, depth: usize) -> Result<(), String> {
        let name = lily_engine_music::music_name(self.vm.heap(), music)
            .unwrap_or("?")
            .to_string();
        let origin = self.property(music, "origin");
        let origin = self.origin(origin);
        if self.seen.contains(&music) {
            self.line(depth, &format!("= {name} {origin}"));
            return Ok(());
        }
        self.seen.push(music);
        self.line(depth, &format!("{name} {origin}"));
        let props = lily_engine_music::mutable_properties(self.vm.heap_mut(), music);
        let mut entries = Vec::new();
        for entry in self.list(props).unwrap_or_default() {
            let heap = self.vm.heap();
            let (Some(key), Some(value)) = (heap.car(entry), heap.cdr(entry)) else {
                continue;
            };
            let key = heap.name(key).unwrap_or_default().to_string();
            if key != "origin" {
                entries.push((key, value));
            }
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (key, value) in entries {
            if self.is_music(value) {
                self.line(depth + 1, &format!("{key}:"));
                self.node(value, depth + 2)?;
                continue;
            }
            match self.list(value) {
                Some(items) if !items.is_empty() && items.iter().all(|&v| self.is_music(v)) => {
                    self.line(depth + 1, &format!("{key}:"));
                    for item in items {
                        self.node(item, depth + 2)?;
                    }
                }
                _ => {
                    let text = self.value(value)?;
                    self.line(depth + 1, &format!("{key} = {text}"));
                }
            }
        }
        Ok(())
    }

    /// `(name v)` in `(lily)`.
    fn call(&mut self, name: &str, v: Value) -> Result<Value, String> {
        let text = format!("(@@ (lily) {name})");
        self.vm
            .eval_text(&text, None)
            .and_then(|f| self.vm.apply(f, &[v]))
            .map_err(|e| e.text(self.vm.heap()))
    }

    /// `music-value`: a value as `write` writes it, but origins as above,
    /// procedures by their names and music by its type's.
    fn value(&mut self, v: Value) -> Result<String, String> {
        let heap = self.vm.heap();
        if heap.foreign::<lily_engine_music::Input>(v).is_some() {
            return Ok(self.origin(v));
        }
        if heap.is_procedure(v) {
            let name = self
                .vm
                .eval_text("procedure-name", None)
                .and_then(|f| self.vm.apply(f, &[v]))
                .map_err(|e| e.text(self.vm.heap()))?;
            let mut text = String::new();
            self.vm.heap().write(name, &mut text);
            return Ok(match name {
                Value::FALSE => String::from("#<procedure>"),
                _ => format!("#<procedure {text}>"),
            });
        }
        if self.is_music(v) {
            let name = lily_engine_music::music_name(heap, v).unwrap_or("?");
            return Ok(format!("#<music {name}>"));
        }
        if self.call("ly:unpure-pure-container?", v)? != Value::FALSE {
            let unpure = self.call("ly:unpure-pure-container-unpure-part", v)?;
            let pure = self.call("ly:unpure-pure-container-pure-part", v)?;
            let (unpure, pure) = (self.value(unpure)?, self.value(pure)?);
            return Ok(format!("#<unpure-pure-container {unpure} {pure}>"));
        }
        let heap = self.vm.heap();
        if let (Some(car), Some(cdr)) = (heap.car(v), heap.cdr(v)) {
            let mut parts = vec![self.value(car)?];
            let mut p = cdr;
            loop {
                let heap = self.vm.heap();
                match (heap.car(p), heap.cdr(p)) {
                    (Some(car), Some(cdr)) => {
                        parts.push(self.value(car)?);
                        p = cdr;
                    }
                    _ if p == Value::NULL => return Ok(format!("({})", parts.join(" "))),
                    _ => {
                        let tail = self.value(p)?;
                        return Ok(format!("({} . {tail})", parts.join(" ")));
                    }
                }
            }
        }
        let mut text = String::new();
        heap.write(v, &mut text);
        Ok(text)
    }
}

/// LilyPond's trees of `text`, compiled as `name` in a directory of its
/// own with `music.ly`.
#[allow(dead_code)]
pub fn lilypond_trees(name: &str, text: &str) -> String {
    let dir = std::env::temp_dir().join(format!("lily-music-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(name);
    std::fs::write(&file, text).unwrap();
    let settings = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/compare/music.ly");
    let out = dir.join("music.txt");
    let run = Command::new("lilypond")
        .args(["--loglevel=WARNING", "-dno-print-pages"])
        .arg(format!("-dinclude-settings={}", settings.display()))
        .arg("-o")
        .arg(dir.join("out"))
        .arg(&file)
        .current_dir(&dir)
        .env("MUSIC_OUT", &out)
        .env("LANGUAGE", "en")
        .output()
        .expect("lilypond runs");
    let trees = std::fs::read_to_string(&out).unwrap_or_default();
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    trees
}

/// Asserts `got` equals `expected` line by line, naming the first line
/// that differs.
#[allow(dead_code)]
pub fn same(expected: &str, got: &str, what: &str) {
    let (e, g): (Vec<_>, Vec<_>) = (expected.lines().collect(), got.lines().collect());
    if let Some(at) = (0..e.len().max(g.len())).find(|&i| e.get(i) != g.get(i)) {
        panic!(
            "{what}: line {}: expected {:?}, got {:?}\n--- expected\n{expected}--- got\n{got}",
            at + 1,
            e.get(at),
            g.get(at)
        );
    }
}
