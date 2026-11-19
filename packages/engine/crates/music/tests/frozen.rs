//! N2 part 1 (04 §6.1–§6.2): the music types' table, frozen music behind
//! D65's face, thawing and digests, and the placeholders of D67. The table
//! comes from the installed LilyPond 2.26.0 when the crate is built; without
//! it the table is empty, these tests are skipped, and nothing is verified.
use std::path::PathBuf;
use std::process::Command;

use lily_engine_music::{
    CALLBACKS, EXTENSION, PLACEHOLDERS, RELATIVE_CALLBACKS, TYPES, freeze, frozen_node, is_frozen,
    kind,
};
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

/// A VM whose `(lily)` has the music types' table, with what it needs of
/// `lily-library.scm` and `define-music-callbacks.scm` loaded first; None
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
    Some(vm)
}

fn eval(vm: &mut Vm, text: &str) -> Value {
    vm.eval_text(text, None)
        .unwrap_or_else(|e| panic!("{text}: {}", e.text(vm.heap())))
}

fn write(vm: &Vm, v: Value) -> String {
    let mut out = String::new();
    vm.heap().write(v, &mut out);
    out
}

/// What `text` gives, written.
fn show(vm: &mut Vm, text: &str) -> String {
    let v = eval(vm, text);
    write(vm, v)
}

/// Freezes the music `name` holds; whether it froze.
fn freeze_global(vm: &mut Vm, name: &str) -> bool {
    let m = eval(vm, name);
    freeze(vm.heap_mut(), m)
}

fn frozen(vm: &mut Vm, name: &str) -> bool {
    let m = eval(vm, name);
    is_frozen(vm.heap(), m)
}

/// The digest of the root of the frozen music `name` holds.
fn digest(vm: &mut Vm, name: &str) -> u64 {
    let m = eval(vm, name);
    let (tree, node) = frozen_node(vm.heap(), m).unwrap();
    tree.nodes()[node as usize].digest
}

const MUSIC: &str = "(make-music 'SequentialMusic
  'elements (list (make-music 'NoteEvent 'duration (ly:make-duration 2)
                              'pitch (ly:make-pitch 0 1 1/2)
                              'articulations (list (make-music 'ArticulationEvent
                                                               'articulation-type 'staccato)))
                  (make-music 'RestEvent 'duration (ly:make-duration 3)))
  'tweaks '((color . red)))";

/// The 131 types, sorted as `music-descriptions` is, and every C++
/// callback their properties name a placeholder or one of the lengths' and
/// starts', which all but the default one are.
#[test]
fn the_table_holds_the_131_types_and_their_callbacks() {
    if TYPES.is_empty() {
        eprintln!("skipped: no music types without LilyPond 2.26.0");
        return;
    }
    assert_eq!(TYPES.len(), 131);
    assert!(TYPES.windows(2).all(|w| w[0].name < w[1].name));
    assert_eq!(kind("AbsoluteDynamicEvent"), Some(0));
    let note = &TYPES[usize::from(kind("NoteEvent").unwrap())];
    assert_eq!(
        note.types,
        ["event", "note-event", "rhythmic-event", "melodic-event"]
    );
    let mut named: Vec<&str> = TYPES
        .iter()
        .flat_map(|t| t.props.iter())
        .map(|&(_, value)| value)
        .filter(|value| value.starts_with("ly:"))
        .collect();
    named.sort_unstable();
    named.dedup();
    // The quote iterator's constructor is named by `quote-substitute`, not
    // by a type.
    let mut callbacks: Vec<&str> = PLACEHOLDERS
        .iter()
        .map(|p| p.name)
        .filter(|&name| name != "ly:quote-iterator::constructor")
        .collect();
    callbacks.extend(&CALLBACKS[1..]);
    callbacks.extend(&RELATIVE_CALLBACKS);
    callbacks.sort_unstable();
    assert_eq!(named, callbacks);
}

/// `make-music` gives a type's immutable properties as LilyPond's table
/// has them, its C++ callbacks D67's placeholders.
#[test]
fn music_has_its_types_immutable_properties() {
    let Some(mut vm) = vm() else { return };
    assert_eq!(
        show(
            &mut vm,
            "(ly:music-property (make-music 'NoteEvent) 'types)"
        ),
        "(event note-event rhythmic-event melodic-event)"
    );
    assert_eq!(
        show(
            &mut vm,
            "(procedure-name (ly:music-property (make-music 'SequentialMusic) 'iterator-ctor))"
        ),
        "ly:sequential-iterator::constructor"
    );
    // A Scheme callback of define-music-callbacks.scm's, and one of the file.
    assert_eq!(
        show(
            &mut vm,
            "(eq? (ly:music-property (make-music 'UnfoldedRepeatedMusic) 'elements-callback)
                  make-unfolded-set)"
        ),
        "#t"
    );
    assert_eq!(
        show(
            &mut vm,
            "(let ((m (make-music 'SequentialMusic 'elements (list (make-music 'SkipEvent)))))
               (map (lambda (e) (ly:music-property e 'name))
                    ((ly:music-property m 'elements-callback) m)))"
        ),
        "(SkipEvent)"
    );
    let e = vm
        .eval_text(
            "((ly:music-property (make-music 'SequentialMusic) 'iterator-ctor))",
            None,
        )
        .unwrap_err();
    assert_eq!(
        e.text(vm.heap()),
        "In procedure ly:sequential-iterator::constructor: not in the engine before N4"
    );
    let e = vm
        .eval_text(
            "((ly:music-property (make-music 'SequentialMusic) 'length-callback) 1 2)",
            None,
        )
        .unwrap_err();
    assert!(e.text(vm.heap()).contains("Wrong number of arguments"));
}

/// The C++ callbacks of layout that `lily.scm`'s init data stores are
/// placeholders too (`grob.rs`): primitives with LilyPond 2.26.0's names and
/// arities, whose call fails by name.
#[test]
fn layout_callbacks_are_named_placeholders() {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    eval(&mut vm, "(set-current-module (resolve-module '(lily)))");
    // No placeholder has an optional argument since the side positions'
    // went to layout (MS3 step 40), whose test prints them. The balloon's,
    // R9's, since the stem tremolo's went to layout (MS4 step 5).
    assert_eq!(
        show(
            &mut vm,
            "(list ly:balloon-interface::print ly:balloon-interface::pure-height)"
        ),
        "(#<procedure ly:balloon-interface::print (_)> #<procedure ly:balloon-interface::pure-height (_ _ _)>)"
    );
    let e = vm
        .eval_text("(ly:balloon-interface::print 1)", None)
        .unwrap_err();
    assert_eq!(
        e.text(vm.heap()),
        "In procedure ly:balloon-interface::print: not in the engine before layout (MS3)"
    );
}

/// Frozen music prints and reads as the live music it was; reading a list
/// or music thaws a handle, and only that handle.
#[test]
fn frozen_music_shows_scheme_what_live_music_did() {
    let Some(mut vm) = vm() else { return };
    eval(&mut vm, &format!("(define m {MUSIC})"));
    let live = show(&mut vm, "m");
    let props = show(&mut vm, "(ly:music-mutable-properties m)");
    assert!(freeze_global(&mut vm, "m"));
    assert!(frozen(&mut vm, "m"));
    assert_eq!(show(&mut vm, "m"), live);
    assert!(!freeze_global(&mut vm, "m"), "frozen already");

    // A copy is a handle of the same node.
    eval(&mut vm, "(define c (ly:music-deep-copy m))");
    assert!(frozen(&mut vm, "c"));
    assert_eq!(show(&mut vm, "c"), live);

    // What is no list and no music is read from the node.
    assert_eq!(
        show(&mut vm, "(ly:music-property m 'name)"),
        "SequentialMusic"
    );
    assert_eq!(show(&mut vm, "(ly:music-property m 'nothing 5)"), "5");
    assert_eq!(show(&mut vm, "(ly:music? m)"), "#t");
    assert_eq!(show(&mut vm, "(ly:event? m)"), "#f");
    assert!(frozen(&mut vm, "m"));

    // A list thaws the handle; its music are handles of the children.
    eval(
        &mut vm,
        "(define note (car (ly:music-property m 'elements)))",
    );
    assert!(!frozen(&mut vm, "m"));
    assert!(frozen(&mut vm, "note"));
    assert_eq!(
        show(&mut vm, "(eq? note (car (ly:music-property m 'elements)))"),
        "#t"
    );
    assert_eq!(
        show(&mut vm, "(ly:music-property note 'pitch)"),
        "#<Pitch dis' >"
    );
    assert!(frozen(&mut vm, "note"));
    assert_eq!(
        show(
            &mut vm,
            "(map (lambda (a) (ly:music-property a 'articulation-type))
                  (ly:music-property note 'articulations))"
        ),
        "(staccato)"
    );
    assert!(!frozen(&mut vm, "note"));

    // A change is seen through the thawed parent, not by the copy.
    eval(
        &mut vm,
        "(ly:music-set-property! note 'duration (ly:make-duration 1))",
    );
    assert!(show(&mut vm, "m").contains("(duration . #<Duration 2 >)"));
    assert_eq!(show(&mut vm, "c"), live);
    assert_eq!(show(&mut vm, "(ly:music-mutable-properties c)"), props);
    assert!(!frozen(&mut vm, "c"));
    assert_eq!(
        show(
            &mut vm,
            "(eq? (ly:music-mutable-properties c) (ly:music-mutable-properties c))"
        ),
        "#t"
    );

    // Frozen again, with the change.
    let changed = show(&mut vm, "m");
    assert!(freeze_global(&mut vm, "m"));
    assert_eq!(show(&mut vm, "m"), changed);
    assert_eq!(
        show(
            &mut vm,
            "(ly:music-property (car (ly:music-property m 'elements)) 'duration)"
        ),
        "#<Duration 2 >"
    );
}

/// A digest sees a type, its properties and its children, not `origin`.
#[test]
fn digests_see_contents_not_origins() {
    let Some(mut vm) = vm() else { return };
    for (name, text) in [
        ("a", MUSIC.to_string()),
        ("b", MUSIC.to_string()),
        ("other-pitch", MUSIC.replace("0 1 1/2", "0 1 0")),
        ("other-type", MUSIC.replace("'RestEvent", "'SkipEvent")),
        (
            "located",
            format!("(let ((m {MUSIC})) (ly:music-set-property! m 'origin 5) m)"),
        ),
    ] {
        eval(&mut vm, &format!("(define {name} {text})"));
        assert!(freeze_global(&mut vm, name), "{name}");
    }
    let a = digest(&mut vm, "a");
    assert_eq!(digest(&mut vm, "b"), a);
    assert_eq!(digest(&mut vm, "located"), a);
    assert_ne!(digest(&mut vm, "other-pitch"), a);
    assert_ne!(digest(&mut vm, "other-type"), a);
    // Frozen music in live music is copied into its tree, digest and all.
    eval(
        &mut vm,
        "(define outer (make-music 'SimultaneousMusic 'elements (list a)))",
    );
    assert!(freeze_global(&mut vm, "outer"));
    let outer = eval(&mut vm, "outer");
    let (tree, _) = frozen_node(vm.heap(), outer).unwrap();
    assert_eq!(tree.nodes()[1].digest, a);
    assert_eq!(tree.nodes()[0].size, 5);
}

/// What cannot be frozen stays live.
#[test]
fn music_that_cannot_be_frozen_stays_live() {
    let Some(mut vm) = vm() else { return };
    eval(
        &mut vm,
        "(define custom (ly:make-music '((name . Custom) (types))))",
    );
    assert!(!freeze_global(&mut vm, "custom"));
    eval(
        &mut vm,
        "(define cycle (let ((m (make-music 'SequentialMusic)))
                         (ly:music-set-property! m 'element m)
                         m))",
    );
    assert!(!freeze_global(&mut vm, "cycle"));
    assert!(!frozen(&mut vm, "cycle"));
    assert_eq!(
        show(&mut vm, "(eq? cycle (ly:music-property cycle 'element))"),
        "#t"
    );
    assert!(!freeze_global(&mut vm, "5"));
    // A circular list of music is held as a value, not walked for ever.
    eval(
        &mut vm,
        "(define circular (let ((l (list (make-music 'SkipEvent))))
                            (set-cdr! l l)
                            (make-music 'SequentialMusic 'elements l)))",
    );
    assert!(freeze_global(&mut vm, "circular"));
}

/// Values that `write` writes in other shapes print from a node as they
/// did live.
#[test]
fn a_handle_prints_as_its_live_music() {
    let Some(mut vm) = vm() else { return };
    eval(
        &mut vm,
        "(define m (make-music 'NoteEvent 'quoted ''x 'pair '(a . b) 'none '()
                               'text \"t\" 'element (make-music 'SkipEvent)))",
    );
    let live = show(&mut vm, "m");
    assert!(freeze_global(&mut vm, "m"));
    assert_eq!(show(&mut vm, "m"), live);
}

/// Frozen music outlives a collection and an image restored natively
/// (step 3 checks it in wasm).
#[test]
fn frozen_music_is_kept_and_saved() {
    let Some(mut vm) = vm() else { return };
    eval(&mut vm, &format!("(define m {MUSIC})"));
    let live = show(&mut vm, "m");
    assert!(freeze_global(&mut vm, "m"));
    eval(&mut vm, "(gc)");
    assert_eq!(show(&mut vm, "m"), live);
    let image = vm.write_image().unwrap();
    let mut restored = Vm::restore(&image, &[&EXTENSION]).unwrap();
    assert!(frozen(&mut restored, "m"));
    assert_eq!(show(&mut restored, "m"), live);
    assert_eq!(
        show(
            &mut restored,
            "(map (lambda (e) (ly:music-property e 'name)) (ly:music-property m 'elements))"
        ),
        "(NoteEvent RestEvent)"
    );
}
