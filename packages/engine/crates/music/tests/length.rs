//! N2 part 2 (04 §6.1, §6.3): lengths and starts, `equal?`, origins as
//! input locations, identities, and frozen music in an image, natively.
//! `tools/bench/test/image.test.mjs` checks lengths, starts, `equal?` and
//! the walkers against LilyPond 2.26.0, natively and in wasm; these check
//! what that probe cannot show. The types' table comes from the installed
//! LilyPond 2.26.0; without it these tests are skipped, and nothing is
//! verified.
use std::path::PathBuf;
use std::process::Command;

use lily_engine_music::{EXTENSION, Input, TYPES, first_difference, frozen_node, is_frozen};
use lily_engine_scheme::{Value, Vm};

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

/// A VM whose `(lily)` has the music types' table and a few helpers; None
/// without the table.
fn vm() -> Option<Vm> {
    if TYPES.is_empty() {
        eprintln!("skipped: no music types without LilyPond 2.26.0");
        return None;
    }
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    vm.eval_text(
        "(set-current-module (resolve-module '(lily)))
         (define-public INF-MOMENT (ly:make-moment +inf.0))
         (define-public ZERO-MOMENT (ly:make-moment 0 1))",
        None,
    )
    .unwrap();
    let callbacks = lily_scm().unwrap().join("define-music-callbacks.scm");
    let text = std::fs::read_to_string(&callbacks).unwrap();
    vm.eval_text(&text, callbacks.to_str()).unwrap();
    lily_engine_music::define_music_types(&mut vm).unwrap();
    eval(
        &mut vm,
        "(define (note step log)
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 step 0) 'duration (ly:make-duration log)))
         (define (seq . elements) (make-music 'SequentialMusic 'elements elements))",
    );
    Some(vm)
}

fn eval(vm: &mut Vm, text: &str) -> Value {
    vm.eval_text(text, None)
        .unwrap_or_else(|e| panic!("{text}: {}", e.text(vm.heap())))
}

/// What `text` gives, written, or its error's key and text.
fn show(vm: &mut Vm, text: &str) -> String {
    match vm.eval_text(text, None) {
        Ok(v) => {
            let mut out = String::new();
            vm.heap().write(v, &mut out);
            out
        }
        Err(e) => format!(
            "{}: {}",
            vm.heap().name(e.key).unwrap_or("?"),
            e.text(vm.heap())
        ),
    }
}

/// An input location as the parser makes one, bound to `name`.
fn define_input(vm: &mut Vm, name: &str, start: u32, end: u32) {
    let input = Input::new(0, start, end, "a.ly".into(), [1, start, start]);
    let v = vm.heap_mut().wrap(input);
    vm.define(name, v);
}

/// Lengths and starts read frozen music without thawing it. A length is
/// kept for a frozen node only when it read no live music: music that a
/// mixed list holds stays live, and a change to it is seen.
#[test]
fn lengths_read_frozen_music_and_see_live_music_change() {
    let Some(mut vm) = vm() else { return };
    eval(
        &mut vm,
        "(define s (%music-freeze! (seq (note 0 2) (note 1 3))))
         (define n (note 2 2))
         (define mixed (%music-freeze! (seq n 'no-music)))",
    );
    assert_eq!(
        show(&mut vm, "(list (ly:music-length s) (ly:music-start s))"),
        "(#<Mom 3/8> #<Mom 0>)"
    );
    assert_eq!(show(&mut vm, "(ly:music-length s)"), "#<Mom 3/8>");
    assert_eq!(show(&mut vm, "(%music-frozen? s)"), "#t");
    assert_eq!(show(&mut vm, "(ly:music-length mixed)"), "#<Mom 1/4>");
    eval(
        &mut vm,
        "(ly:music-set-property! n 'duration (ly:make-duration 1))",
    );
    assert_eq!(show(&mut vm, "(ly:music-length mixed)"), "#<Mom 1/2>");
    assert_eq!(
        show(&mut vm, "(list (%music-frozen? mixed) (%music-frozen? n))"),
        "(#t #f)"
    );
}

/// A callback of Scheme is called in the primitive's place, from Scheme and
/// from [`Vm::apply`], and what it throws is thrown. The frozen music
/// around it stays frozen.
#[test]
fn a_scheme_callback_runs_in_the_primitives_place() {
    let Some(mut vm) = vm() else { return };
    eval(
        &mut vm,
        "(define (custom callback)
           (ly:make-music (cons (cons 'length-callback callback)
                                (assq-ref %music-properties 'SkippedMusic))))
         (define c (%music-freeze! (custom (lambda (m) (ly:make-moment 3/4)))))
         (define outer (%music-freeze! (seq (note 0 2) (custom (lambda (m) (ly:make-moment 3/4))))))
         (define failing (seq (custom (lambda (m) (throw 'oops m)))))",
    );
    assert_eq!(
        show(&mut vm, "(list (%music-frozen? c) (%music-frozen? outer))"),
        "(#t #t)"
    );
    assert_eq!(show(&mut vm, "(ly:music-length c)"), "#<Mom 3/4>");
    assert_eq!(show(&mut vm, "(ly:music-length outer)"), "#<Mom 1>");
    assert_eq!(
        show(
            &mut vm,
            "(ly:music-sequence::cumulative-length-callback outer)"
        ),
        "#<Mom 1>"
    );
    assert_eq!(show(&mut vm, "(%music-frozen? outer)"), "#t");
    let length = eval(&mut vm, "ly:music-length");
    let outer = eval(&mut vm, "outer");
    let m = vm.apply(length, &[outer]).unwrap();
    let mut out = String::new();
    vm.heap().write(m, &mut out);
    assert_eq!(out, "#<Mom 1>");
    assert!(
        show(&mut vm, "(ly:music-length failing)").starts_with("oops: "),
        "{}",
        show(&mut vm, "(ly:music-length failing)")
    );
    // The primitive's place is taken once; the next call is its own.
    assert_eq!(show(&mut vm, "(ly:music-length (note 0 1))"), "#<Mom 1/2>");
}

/// A length that does not fit is LilyPond's wrapped one; the engine's
/// is an overflow (D61).
#[test]
fn a_length_that_does_not_fit_is_an_overflow() {
    let Some(mut vm) = vm() else { return };
    assert_eq!(
        show(
            &mut vm,
            "(let ((long (make-music 'NoteEvent 'duration (ly:make-duration 0 0 (expt 2 62)))))
               (ly:music-length (%music-freeze! (seq long (ly:music-deep-copy long) (ly:music-deep-copy long)))))"
        ),
        "numerical-overflow: Throw to key `numerical-overflow' with args `(\"ly:music-length\" \"Numerical overflow\" () #f)'."
    );
}

/// `equal?` is `Prob::equal_p`: entry by entry in order, `origin` left out,
/// frozen and live alike, and so is a digest.
#[test]
fn equal_leaves_out_origins_and_reads_frozen_music() {
    let Some(mut vm) = vm() else { return };
    define_input(&mut vm, "here", 3, 7);
    define_input(&mut vm, "there", 40, 44);
    eval(
        &mut vm,
        "(define (located loc) (let ((m (seq (note 0 2) (note 1 2))))
           (ly:music-set-property! m 'origin loc)
           (ly:music-set-property! (car (ly:music-property m 'elements)) 'origin loc)
           m))
         (define a (located here))
         (define b (located there))
         (define fa (%music-freeze! (located here)))
         (define fb (%music-freeze! (located there)))
         (define unlocated (%music-freeze! (seq (note 0 2) (note 1 2))))",
    );
    assert_eq!(
        show(
            &mut vm,
            "(list (equal? a b) (equal? fa b) (equal? fa fb) (equal? fb a) (equal? fa unlocated)
                   (equal? (car (ly:music-property a 'elements)) (note 0 2))
                   (equal? fa (seq (note 0 2) (note 1 4))) (equal? fa (seq (note 0 2))))"
        ),
        "(#t #t #t #t #t #t #f #f)"
    );
    // A circular list against frozen children ends, unequal.
    assert_eq!(
        show(
            &mut vm,
            "(let ((l (list (note 0 2) (note 1 2))))
               (set-cdr! (cdr l) l)
               (equal? fa (make-music 'SequentialMusic 'elements l 'origin here)))"
        ),
        "#f"
    );
    let digest = |vm: &mut Vm, name: &str| {
        let m = eval(vm, name);
        let (tree, node) = frozen_node(vm.heap(), m).unwrap();
        tree.nodes()[node as usize].digest
    };
    assert_eq!(digest(&mut vm, "fa"), digest(&mut vm, "fb"));
    assert_eq!(digest(&mut vm, "fa"), digest(&mut vm, "unlocated"));
    // The origin is a location with its span, read without thawing.
    assert_eq!(show(&mut vm, "(ly:input-location? here)"), "#t");
    assert_eq!(
        show(&mut vm, "(ly:music-property fb 'origin)"),
        "#<location a.ly:1:41>"
    );
    let fb = eval(&mut vm, "fb");
    let (tree, node) = frozen_node(vm.heap(), fb).unwrap();
    assert_eq!(tree.origin(vm.heap(), node), Some((0, 40, 44)));
    assert_eq!(tree.origin(vm.heap(), node + 2), None);
    assert!(is_frozen(vm.heap(), fb));
}

/// `ly:music-deep-copy`'s origin is set as `set_origin` sets it: an entry
/// replaced where it is, music in lists, an origin taken from music.
#[test]
fn a_copys_origin_is_set_as_set_origin_sets_it() {
    let Some(mut vm) = vm() else { return };
    define_input(&mut vm, "here", 3, 7);
    define_input(&mut vm, "there", 40, 44);
    eval(
        &mut vm,
        "(define m (note 0 2))
         (ly:music-set-property! m 'origin here)
         (ly:music-set-property! m 'tweaks '())
         (define f (%music-freeze! (ly:music-deep-copy m)))",
    );
    assert_eq!(
        show(
            &mut vm,
            "(map car (ly:music-mutable-properties (ly:music-deep-copy m there)))"
        ),
        "(tweaks origin duration pitch)"
    );
    assert_eq!(
        show(
            &mut vm,
            "(let ((c (ly:music-deep-copy (list (note 0 2) (list m) 5) there)))
               (list (ly:music-property (car c) 'origin) (ly:music-property (caadr c) 'origin) (caddr c)))"
        ),
        "(#<location a.ly:1:41> #<location a.ly:1:41> 5)"
    );
    assert_eq!(
        show(
            &mut vm,
            "(list (ly:music-property (ly:music-deep-copy f m) 'origin)
                   (ly:music-property f 'origin) (%music-frozen? f)
                   (ly:music-property (ly:music-deep-copy (note 0 2) '()) 'origin)
                   (ly:music-property (ly:music-deep-copy (note 0 2) (note 1 2)) 'origin))"
        ),
        "(#<location a.ly:1:4> #<location a.ly:1:4> #t () ())"
    );
    assert_eq!(
        show(&mut vm, "(ly:music-deep-copy m 3)"),
        "wrong-type-arg: In procedure ly:music-deep-copy: Wrong type argument in position 2 (expecting Input): 3"
    );
}

/// 04 §6.3: where two trees first differ, and identities that equal
/// siblings tell apart and moved music keeps.
#[test]
fn trees_are_compared_and_their_nodes_named_by_digests() {
    let Some(mut vm) = vm() else { return };
    define_input(&mut vm, "here", 3, 7);
    eval(
        &mut vm,
        "(define old (%music-freeze! (seq (note 0 2) (note 1 2) (note 0 2))))
         (define same (%music-freeze! (let ((m (seq (note 0 2) (note 1 2) (note 0 2))))
                                        (ly:music-set-property! m 'origin here) m)))
         (define new (%music-freeze! (seq (note 0 2) (note 3 2) (note 0 2))))
         (define props (%music-freeze! (let ((m (seq (note 0 2) (note 1 2) (note 0 2))))
                                         (ly:music-set-property! m 'tweaks '((color . red))) m)))",
    );
    let [old, same, new, props] = ["old", "same", "new", "props"].map(|n| eval(&mut vm, n));
    let heap = vm.heap();
    let tree = |m| frozen_node(heap, m).unwrap().0;
    let (old, same, new, props) = (tree(old), tree(same), tree(new), tree(props));
    assert_eq!(first_difference(heap, old, same), None);
    assert_eq!(first_difference(heap, old, new), Some((2, 2)));
    assert_eq!(first_difference(heap, old, props), Some((0, 0)));
    let children: Vec<u32> = old.children(0).collect();
    assert_eq!(children, [1, 2, 3]);
    assert_ne!(old.identity(1), old.identity(3));
    for i in 0..4 {
        assert_eq!(old.identity(i), same.identity(i));
    }
    assert_ne!(old.identity(2), new.identity(2));
}

/// An image holds frozen music's origins, and gives lengths after a
/// restore as before it.
#[test]
fn origins_and_lengths_survive_an_image() {
    let Some(mut vm) = vm() else { return };
    define_input(&mut vm, "here", 3, 7);
    eval(
        &mut vm,
        "(define m (%music-freeze! (let ((m (seq (note 0 2) (note 1 3))))
                                     (ly:music-set-property! m 'origin here) m)))
         (define first-length (ly:music-length m))",
    );
    let image = vm.write_image().unwrap();
    let mut restored = Vm::restore(&image, &[&EXTENSION]).unwrap();
    assert_eq!(
        show(
            &mut restored,
            "(list (%music-frozen? m) (ly:music-length m) first-length (ly:music-property m 'origin))"
        ),
        "(#t #<Mom 3/8> #<Mom 3/8> #<location a.ly:1:4>)"
    );
    let m = eval(&mut restored, "m");
    let (tree, node) = frozen_node(restored.heap(), m).unwrap();
    assert_eq!(tree.origin(restored.heap(), node), Some((0, 3, 7)));
}
