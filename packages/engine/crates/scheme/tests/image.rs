//! S4, step 12a: a VM written to an image and restored natively.
//!
//! - A VM given every kind of object the heap has, and every foreign type of
//!   the Scheme crate and the music crate, gives the same answers to a probe
//!   after it is restored, and so does the restored VM after a collection
//!   that reuses the free slots. The restored VM writes the same image again.
//! - What the image refuses: a VM not at rest, a foreign type that cannot be
//!   saved, an image of another format, primitive tables that differ, a
//!   foreign type with no loader, and an image cut short.
//! - `lily-library.scm` and `music-functions.scm` loaded from the installed
//!   LilyPond 2.26.0's `scm/lily/` as `tests/library.rs` loads them: the
//!   probe `tools/bench/probe-library.scm` gives the same output from source
//!   and from the restored image. Skipped without LilyPond 2.26.0.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use lily_engine_music::EXTENSION;
use lily_engine_scheme::{
    Exception, Extension, Foreign, Heap, HostHandler, ImageError, ImageReader, ImageWriter, Loader,
    Value, Vm, read,
};

/// Gives a VM objects of every kind: what [`PROBE`] then reads.
const STATE: &str = r#"
(use-modules (oop goops) (ice-9 binary-ports) (srfi srfi-1))
(define data (list 1 2.5 "strasse straße" #\x 'sym #:kw (vector 1 '(2)) (expt 2 80)
                   (- (expt 2 70)) 3/7 (/ (expt 2 70) 3) (make-rectangular 1.5 -2) '()))
(define table (make-hash-table))
(hash-set! table "a" 1)
(hashq-set! table 'b 2)
(hashv-set! table 3.5 3)
(hashq-set! table data 'data)
(define weak (make-weak-key-hash-table))
(hashq-set! weak data 'kept)
(hashq-set! weak (list 'dies) 'gone)
(define-syntax swap!
  (syntax-rules ()
    ((_ a b) (let ((t a)) (set! a b) (set! b t)))))
(defmacro twice (x) `(begin ,x ,x))
(define-macro (thrice x) `(begin ,x ,x ,x))
(define counter (let ((n 0)) (lambda () (set! n (+ n 1)) n)))
(define param (make-parameter 10 (lambda (x) (* x 2))))
(define fluid (make-fluid 5))
(define prop (make-object-property))
(set! (prop counter) 'counted)
(define first-of (make-procedure-with-setter car set-car!))
(define in (open-input-string "(a b) c d"))
(read in)
(define out (open-output-string))
(display "partial" out)
(define-class <point> () (x #:init-keyword #:x) (y #:init-value 0))
(define point (make <point> #:x 3))
(define bytes
  (call-with-values open-bytevector-output-port
    (lambda (p take) (put-u8 p 7) (put-u8 p 255) (take))))
(define bytes-port (call-with-values open-bytevector-output-port (lambda (p take) p)))
(define array '#2@1@0((1 2) (3 4)))
(define symbol (gensym "g"))
(define regex ((@@ (lily) ly:make-regex) "a+"))
(define moment ((@@ (lily) ly:make-moment) 3/4 -1/8))
(define duration ((@@ (lily) ly:make-duration) 3 1 2/3))
(define pitch ((@@ (lily) ly:make-pitch) 1 2 1/2))
(define scale ((@@ (lily) ly:make-scale) #(0 1 2 3 4)))
(define cells (make-vector 3 0))
(define (bump! i) (vector-set! cells i (+ 1 (vector-ref cells i))) (vector-copy cells))
(define* (keys a #:optional (b 2) #:key (c 3)) (list a b c))
(define-module (probe module) #:export (exported))
(define exported 42)
(define hidden 7)
(set-current-module (resolve-module '(guile-user)))
(use-modules (probe module))
"#;

/// Reads what [`STATE`] made, using it as the program goes.
const PROBE: &str = r#"
(define (probe form)
  (catch #t
    (lambda () (write (primitive-eval form)))
    (lambda (key . args) (display key) (display " ") (write args)))
  (newline))
(probe 'data)
(probe '(list (hash-ref table "a") (hashq-ref table 'b) (hashv-ref table 3.5) (hashq-ref table data)
              (hash-map->list (lambda (k v) v) table)))
(probe '(list (hashq-ref weak data) (hash-count (const #t) weak)))
(probe '(let ((a 1) (b 2)) (swap! a b) (list a b)))
(probe '(let ((n 0)) (twice (set! n (+ n 1))) (thrice (set! n (* n 2))) n))
(probe '(list (counter) (counter) (prop counter)))
(probe '(list (param) (parameterize ((param 3)) (param)) (fluid-ref fluid)))
(probe '(let ((l (list 1 2))) (set! (first-of l) 9) (list (first-of l) l)))
(probe '(list (read in) (read in) (get-output-string out)))
(probe '(list point (slot-ref point 'x) (slot-ref point 'y) (class-name <point>) (is-a? point <point>)))
(probe '(list bytes (begin (put-u8 bytes-port 1) bytes-port)))
(probe 'array)
(probe '(list (symbol? symbol) (eq? symbol (string->symbol (symbol->string symbol))) (gensym "g")))
(probe '(list regex moment duration pitch scale))
(probe '(list ((@@ (lily) ly:moment-add) moment moment) ((@@ (lily) ly:pitch-transpose) pitch pitch)))
(probe '(list (bump! 1) (bump! 1) (keys 1) (keys 1 5 #:c 6) keys))
(probe '(list exported (@ (probe module) exported) (@@ (probe module) hidden)))
(probe '(@ (probe module) hidden))
(probe '(list (current-module) (module-name (resolve-module '(probe module)))))
(probe '(list (fold cons '() '(1 2 3)) (map + '(1 2) '(10 20)) (call-with-values (lambda () (values 1 2)) list)))
(probe '(string-ref (list-ref data 2) 12))
(probe '(car 5))
(probe 'unbound-variable)
"#;

/// What [`PROBE`] gives after [`STATE`], from source and from the image.
const EXPECTED: &str = r#"(1 2.5 "strasse straße" #\x sym #:kw #(1 (2)) 1208925819614629174706176 -1180591620717411303424 3/7 1180591620717411303424/3 1.5-2.0i ())
(1 2 3 data (1 2 3 data))
(kept 1)
(2 1)
16
(1 2 counted)
(20 6 5)
(9 (9 2))
(c d "partial")
(#<<point>> 3 0 <point> #t)
(#vu8(7 255) #<output: file>)
#2@1@0((1 2) (3 4))
(#t #f g2)
(#<Regex> #<Mom 3/4G-1/8> #<Duration 8.*2/3 > #<Pitch eis'' > #<Scale>)
(#<Mom 3/2G-1/4> #<Pitch g??''' >)
(#(0 1 0) #(0 2 0) (1 2 3) (1 5 6) #<procedure keys args>)
(42 42 7)
unbound-variable ("module-lookup" "Unbound variable: ~S" (hidden) #f)
(#<directory (guile-user)> (probe module))
((3 2 1) (11 22) (1 2))
#\ß
wrong-type-arg ("car" "Wrong type (expecting ~A): ~S" ("pair" 5) (5))
unbound-variable (#f "Unbound variable: ~S" (unbound-variable) #f)
"#;

/// A VM with [`STATE`] and the music crate.
fn state() -> Vm {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    if let Err(e) = vm.eval_text(STATE, None) {
        panic!("the state does not load: {}", e.text(vm.heap()));
    }
    vm.take_output();
    vm
}

fn probe(vm: &mut Vm, program: &str) -> String {
    if let Err(e) = vm.eval_text(program, None) {
        panic!("the probe does not run: {}", e.text(vm.heap()));
    }
    vm.take_output()
}

#[test]
fn a_restored_vm_gives_what_the_written_one_gives() {
    let mut vm = state();
    let image = vm.write_image().unwrap();
    let mut restored = Vm::restore(&image, &[&EXTENSION]).unwrap();
    let again = restored.write_image().unwrap();
    assert!(image == again, "the restored VM writes the same image");
    let from_source = probe(&mut vm, PROBE);
    let from_image = probe(&mut restored, PROBE);
    assert_eq!(from_source, from_image);
    assert_eq!(from_image, EXPECTED);
}

/// `read-hash-extend` with `#\{`, as `parser-ly-from-scheme.scm` calls it,
/// makes `#{ … #}` read as LilyPond's `read-lily-expression` reads it, in
/// files and in `read`, and a restored VM keeps it.
#[test]
fn lilypond_braces_read_after_read_hash_extend_and_after_a_restore() {
    let lily = "((@@ (lily) read-lily-expression-internal) \" c4 \" #f 0 (list))";
    let mut vm = Vm::new();
    assert_eq!(
        probe(&mut vm, "(write (read (open-input-string \"#{a b}#\")))"),
        "#{a b}#"
    );
    probe(&mut vm, "(read-hash-extend #\\{ (lambda (c port) #f))");
    let read = "(write (read (open-input-string \"#{ c4 #}\")))";
    assert_eq!(probe(&mut vm, read), lily);
    assert_eq!(
        probe(&mut vm, "(write (car '#{ c4 #}))"),
        "(@@ (lily) read-lily-expression-internal)"
    );
    let image = vm.write_image().unwrap();
    let mut restored = Vm::restore(&image, &[]).unwrap();
    assert_eq!(probe(&mut restored, read), lily);
    let e = vm
        .eval_text("(read-hash-extend #\\a car)", None)
        .unwrap_err();
    assert_eq!(
        e.text(vm.heap()),
        "In procedure read-hash-extend: the engine's reader extends # with LilyPond's #{ only"
    );
}

#[test]
fn a_restored_vm_collects_and_allocates_into_its_free_slots() {
    let mut vm = state();
    let image = vm.write_image().unwrap();
    let mut restored = Vm::restore(&image, &[&EXTENSION]).unwrap();
    let before = restored.heap().objects();
    // Garbage enough for collections at safe points, then one by hand.
    let churn = "(let loop ((i 0) (acc '())) (if (< i 300000) (loop (+ i 1) (if (= 0 (remainder i 1000)) '() (cons (list i (number->string i)) acc))) (length acc)))";
    probe(&mut restored, churn);
    restored.collect();
    assert!(restored.heap().stats().collections > 0);
    assert!(restored.heap().objects() < before * 2);
    let mut fresh = Vm::restore(&image, &[&EXTENSION]).unwrap();
    assert_eq!(probe(&mut restored, PROBE), probe(&mut fresh, PROBE));
}

/// A foreign type of the test's: savable, with its loader given as an
/// extension of no primitives.
struct Probe(u32);

impl Foreign for Probe {
    fn type_name(&self) -> &'static str {
        "probe"
    }

    fn print(&self, _: &Heap, out: &mut String) {
        out.push_str(&format!("#<probe {}>", self.0));
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.u32(self.0);
        true
    }
}

fn load_probe(r: &mut ImageReader<'_>) -> Option<Box<dyn Foreign>> {
    Some(Box::new(Probe(r.u32()?)))
}

static PROBE_EXTENSION: Extension = Extension {
    operators: None,
    hosts: &[],
    prims: &[],
    loaders: &[Loader {
        name: "probe",
        load: load_probe,
    }],
};

/// One that is not.
struct Unsaved;

impl Foreign for Unsaved {
    fn type_name(&self) -> &'static str {
        "unsaved"
    }
}

#[test]
fn the_image_refuses_what_it_cannot_hold() {
    let mut vm = Vm::new();
    let value = vm.heap_mut().wrap(Probe(17));
    vm.define("p", value);
    let image = vm.write_image().unwrap();

    assert_eq!(
        Vm::restore(&image, &[]).err(),
        Some(ImageError::NoLoader("probe".into()))
    );
    assert_eq!(
        Vm::restore(&image, &[&EXTENSION]).err(),
        Some(ImageError::Primitives)
    );
    let mut restored = Vm::restore(&image, &[&PROBE_EXTENSION]).unwrap();
    assert_eq!(probe(&mut restored, "(write p)"), "#<probe 17>");

    let mut wrong = image.clone();
    wrong[0] = b'X';
    assert_eq!(Vm::restore(&wrong, &[]).err(), Some(ImageError::Format));
    for cut in [20, image.len() / 2, image.len() - 1] {
        assert_eq!(
            Vm::restore(&image[..cut], &[&PROBE_EXTENSION]).err(),
            Some(ImageError::Corrupt),
            "cut at {cut}"
        );
    }
    let mut longer = image.clone();
    longer.push(0);
    assert_eq!(
        Vm::restore(&longer, &[&PROBE_EXTENSION]).err(),
        Some(ImageError::Corrupt)
    );

    let root = vm.heap_mut().root(value);
    assert_eq!(vm.write_image().err(), Some(ImageError::NotAtRest));
    vm.heap_mut().unroot_to(0);
    let _ = root;
    let unsaved = vm.heap_mut().wrap(Unsaved);
    vm.define("u", unsaved);
    assert_eq!(vm.write_image().err(), Some(ImageError::Unsaved("unsaved")));
}

/// A handler for `(%host-call 'twice x)`: `x` doubled by running the VM
/// again.
fn twice(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let double = vm.eval_text("(lambda (x) (* 2 x))", None)?;
    vm.apply(double, &args[1..])
}

/// The host: what it was asked, as a symbol.
fn host(_: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    Ok(args[0])
}

static HOSTS: &[HostHandler] = &[HostHandler {
    what: "twice",
    f: twice,
}];

static HOSTED_EXTENSION: Extension = Extension {
    operators: None,
    hosts: HOSTS,
    prims: &[],
    loaders: &[],
};

const NO_HOST: &str = "In procedure %host-call: no host to call";

/// `%host-call` in `(lily)`, written as `display` writes it.
fn host_call(vm: &mut Vm, call: &str) -> String {
    let text = format!("(display ((module-ref (resolve-module '(lily)) '%host-call) {call}))");
    match vm.eval_text(&text, None) {
        Ok(_) => vm.take_output(),
        Err(e) => e.text(vm.heap()),
    }
}

/// D127: a crate's handler answers the `%host-call`s of its `what`, the
/// host the others; neither is in the image, and a restore gives the
/// extensions' handlers again.
#[test]
fn a_host_handler_answers_its_what_before_the_host_and_after_a_restore() {
    let mut vm = Vm::new();
    assert_eq!(host_call(&mut vm, "'twice 21"), NO_HOST);
    vm.add_host_handlers(HOSTS);
    assert_eq!(host_call(&mut vm, "'twice 21"), "42");
    assert_eq!(host_call(&mut vm, "'other"), NO_HOST);
    vm.set_host(Some(host));
    assert_eq!(host_call(&mut vm, "'twice 21"), "42");
    assert_eq!(host_call(&mut vm, "'other 1"), "other");
    // A later handler of the same `what` replaces the earlier.
    vm.add_host_handlers(&[HostHandler {
        what: "twice",
        f: host,
    }]);
    assert_eq!(host_call(&mut vm, "'twice 21"), "twice");

    let image = vm.write_image().unwrap();
    let mut restored = Vm::restore(&image, &[]).unwrap();
    assert_eq!(host_call(&mut restored, "'twice 21"), NO_HOST);
    let mut restored = Vm::restore(&image, &[&HOSTED_EXTENSION]).unwrap();
    assert_eq!(host_call(&mut restored, "'twice 21"), "42");
}

// --- the two files --------------------------------------------------------------------

/// `scm/lily/` of the installed LilyPond 2.26.0, if there is one, as
/// `tests/library.rs` finds it.
fn lily_scm() -> Option<PathBuf> {
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

/// The toplevel form of `file` that begins with `head`, as its text.
fn definition(lily: &Path, file: &str, head: &str) -> String {
    let text = std::fs::read_to_string(lily.join(file)).unwrap();
    let start = text
        .find(head)
        .unwrap_or_else(|| panic!("{file} has no {head}"));
    let mut heap = Heap::new();
    let r = read(&mut heap, &text, start, None).unwrap();
    text[r.start..r.end].to_string()
}

/// Loads the two files into a VM's `(lily)` as `tests/library.rs` does.
fn load_library(lily: &Path) -> Vm {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    vm.provide_module("lily display-lily", "(define-module (lily display-lily))");
    let symbol_list = definition(lily, "c++.scm", "(define-public (symbol-list? x)");
    let prelude = format!("(set-current-module (resolve-module '(lily)))\n{symbol_list}");
    vm.eval_text(&prelude, None).unwrap();
    for file in ["lily-library.scm", "music-functions.scm"] {
        let path = lily.join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        if let Err(e) = vm.eval_text(&text, path.to_str()) {
            panic!("{file} does not load: {}", e.text(vm.heap()));
        }
    }
    vm
}

fn probe_library() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/bench/probe-library.scm");
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn the_two_files_probe_the_same_from_source_and_from_the_image() {
    let Some(lily) = lily_scm() else {
        eprintln!("LilyPond 2.26.0 is not installed: the two files' image is not written");
        return;
    };
    let program = probe_library();
    let mut writer = load_library(&lily);
    let started = Instant::now();
    let image = writer.write_image().unwrap();
    let written = started.elapsed();
    let started = Instant::now();
    let mut restored = Vm::restore(&image, &[&EXTENSION]).unwrap();
    let restoring = started.elapsed();
    eprintln!(
        "the two files' image: {} bytes, {} objects, written in {written:?}, restored in {restoring:?}",
        image.len(),
        restored.heap().objects()
    );
    assert!(
        restored.write_image().unwrap() == image,
        "the restored VM writes the same image"
    );
    // From source, a VM that has not been collected for an image.
    let from_source = probe(&mut load_library(&lily), &program);
    let mut restored = Vm::restore(&image, &[&EXTENSION]).unwrap();
    let from_image = probe(&mut restored, &program);
    let probes = program.lines().filter(|l| l.starts_with("(probe ")).count();
    assert_eq!(from_source.lines().count(), probes);
    assert_eq!(from_source, from_image);
}
