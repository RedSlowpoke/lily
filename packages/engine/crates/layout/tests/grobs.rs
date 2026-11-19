//! Layout's grobs (MS3 step 13, D108): the state layout keeps of each grob
//! record, in the music crate's slot of the record's translation, by the
//! record's number, a class's records and a Scheme engraver's alike, and
//! traced with the translation; and `grob-property.cc`'s lookup through
//! layout's `ly:grob-property`, `ly:grob-set-property!`, `ly:grob-object`
//! and `ly:grob-set-object!`, with callbacks, `calculation-in-progress`
//! and the cause; and `grob-closure.cc`'s chains, through `output-lib.scm`'s
//! `grob::compose-function` and `grob::offset-function`, read from the
//! installed file, with `tools/bench/probe-grobs.scm`'s calls giving what
//! LilyPond gives for them (`tools/compare/grob-calls.ly`); the
//! constructor's defaults, parents, offsets and extents on both axes, and
//! `align-interface.cc`'s `get_pure_child_y_translation`; a grob's class
//! and interfaces (`has_interface`), and an item's break direction, its
//! broken copies and its column, with `item-scheme.cc`'s three, and grob
//! arrays in the object slots, with `grob-array-scheme.cc`'s five, the
//! pointer groups and `dimensions-scheme.cc`'s five, and the pure calls,
//! property, coordinate and height, with what LilyPond 2.26.0 gives for
//! the same calls (MS3 step 14); and the System, the Paper_score and the
//! paper columns as part 3's functions make them (MS3 step 15). Hand-made
//! definitions and a test class, in a VM with the music crate, the music
//! types and layout; without LilyPond 2.26.0's music types these tests are
//! skipped, and nothing is verified.
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;

use lily_engine_layout::align::get_pure_child_y_translation;
use lily_engine_layout::font;
use lily_engine_layout::font_select::{get_default_font, music_font_alist_chain};
use lily_engine_layout::grob::{
    INFINITE_TRANSLATION, add_offset_callback, alist_chain, cause_link, chain_callback,
    chain_offset_callback, common_refpoint, common_refpoint_of_array, extent, get_parent,
    get_print_stencil, get_vertical_axis_group_index, grobs, has_interface, interfaces, origin,
    programming_error, property_data, relative_coordinate, robust_relative_extent, set_parent,
    suicide, translate_axis, vertical_less, warning, with_grob, with_grob_mut,
};
use lily_engine_layout::grob_array::{
    GrobArray, add_grob, add_unordered_grob, count, extract_grob_array, extract_item_array,
    extract_spanner_array, find_grob, get_grob_array, set_ordered,
};
use lily_engine_layout::item::{
    self, Class, break_status_dir, class, find_prebroken_piece, get_column, is_broken, is_item,
    set_class,
};
use lily_engine_layout::script_engraver::make_script_from_event;
use lily_engine_layout::unpure::{
    call_pure_function, get_pure_property, pure_relative_y_coordinate, pure_y_extent,
};
use lily_engine_layout::{Interval, X, Y};
use lily_engine_music::{
    Cause, Input, Kind, OutputDef, PROCESS, STOP, TYPES, Translated, TranslatorClass, Tx, Work,
    grob_of, make_context_def, paper_book, run_translator_with, scaled_layout, unpure_of,
};
use lily_engine_scheme::{Exception, Value, Vm};

type Run<T> = Result<T, Exception>;

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

fn eval(vm: &mut Vm, text: &str) -> Value {
    vm.eval_text(text, None)
        .unwrap_or_else(|e| panic!("{text}: {}", e.text(vm.heap())))
}

fn show(vm: &Vm, v: Value) -> String {
    let mut out = String::new();
    vm.heap().write(v, &mut out);
    out
}

/// Global with the two grobs' descriptions, a Score and a Voice whose
/// `Note_maker` and `Text_maker` make them.
const DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (interfaces note-head-interface grob-interface)))
        (TextScript (direction . 1)
                    (meta (name . TextScript) (interfaces text-script-interface grob-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice)
      (default-child Voice))",
    "((context-name Voice) (translator-type Engraver_group) (consists Note_maker)
      (consists Text_maker))",
];

/// [`DEFS`] with a Voice whose `Glide_maker` makes grobs of each cause.
const GLIDE_DEFS: &[&str] = &[
    DEFS[0],
    DEFS[1],
    "((context-name Voice) (translator-type Engraver_group) (consists Glide_maker))",
];

/// A VM with the music types, layout, the makers of notes, `Text_maker`
/// and the layout of [`DEFS`].
fn vm() -> Option<(Vm, Value)> {
    vm_with(DEFS)
}

/// A VM with the music types, layout, the makers of notes, `Text_maker`,
/// `Glide_maker`, `Rhythm_maker`, `Align_maker` and the layout of `defs`.
fn vm_with(defs: &[&str]) -> Option<(Vm, Value)> {
    if TYPES.is_empty() {
        eprintln!("skipped: no music types without LilyPond 2.26.0");
        return None;
    }
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    lily_engine_layout::install(&mut vm).unwrap();
    eval(
        &mut vm,
        "(set-current-module (resolve-module '(lily)))
         (define-public INF-MOMENT (ly:make-moment +inf.0))
         (define-public ZERO-MOMENT (ly:make-moment 0 1))",
    );
    let callbacks = lily_scm().unwrap().join("define-music-callbacks.scm");
    let text = std::fs::read_to_string(&callbacks).unwrap();
    vm.eval_text(&text, callbacks.to_str()).unwrap();
    lily_engine_music::define_music_types(&mut vm).unwrap();
    // `Text_maker` makes a TextScript of each note and keeps its handle.
    eval(
        &mut vm,
        "(define (note step log)
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 step 0) 'duration (ly:make-duration log)))
         (define (seq . elements) (make-music 'SequentialMusic 'elements elements))
         (define-public (ly:make-event-class class) (list class))
         (define-public made '())
         (ly:register-translator
           (lambda (context)
             (let ((heard #f))
               `((listeners (note-event . ,(lambda (t ev) (set! heard ev))))
                 (process-music
                  . ,(lambda (t)
                       (when heard
                         (set! made (cons (ly:engraver-make-grob t 'TextScript heard) made))
                         (set! heard #f)))))))
           'Text_maker)
         ;; `Glide_maker` makes, of each note, a TextScript of the event, a
         ;; NoteHead of the TextScript and one of no cause, in `glided`.
         (define-public glided '())
         (ly:register-translator
           (lambda (context)
             (let ((heard #f))
               `((listeners (note-event . ,(lambda (t ev) (set! heard ev))))
                 (process-music
                  . ,(lambda (t)
                       (when heard
                         (let* ((text (ly:engraver-make-grob t 'TextScript heard))
                                (head (ly:engraver-make-grob t 'NoteHead text))
                                (none (ly:engraver-make-grob t 'NoteHead '())))
                           (set! glided (append glided (list text head none))))
                         (set! heard #f)))))))
           'Glide_maker)
         ;; `Rhythm_maker` makes, of each note, a NoteHead, a Rest, a Stem,
         ;; two Dots, a NoteColumn and a DotColumn, in `rhythm`.
         (define-public rhythm '())
         (ly:register-translator
           (lambda (context)
             (let ((heard #f))
               `((listeners (note-event . ,(lambda (t ev) (set! heard ev))))
                 (process-music
                  . ,(lambda (t)
                       (when heard
                         (for-each
                          (lambda (name)
                            (set! rhythm (append rhythm (list (ly:engraver-make-grob t name heard)))))
                          '(NoteHead Rest Stem Dots Dots NoteColumn DotColumn))
                         (set! heard #f)))))))
           'Rhythm_maker)
         ;; `Align_maker` makes, of each note, an alignment, two axis
         ;; groups, a column, what keeps a group alive, a break alignment
         ;; and four groups, in `align-made`.
         (define-public align-made '())
         (ly:register-translator
           (lambda (context)
             (let ((heard #f))
               `((listeners (note-event . ,(lambda (t ev) (set! heard ev))))
                 (process-music
                  . ,(lambda (t)
                       (when heard
                         (for-each
                          (lambda (name)
                            (set! align-made (append align-made (list (ly:engraver-make-grob t name heard)))))
                          '(VerticalAlignment VerticalAxisGroup VerticalAxisGroup
                            NonMusicalPaperColumn NoteHead NoteHead DynamicText
                            DynamicLineSpanner BreakAlignment BreakAlignGroup
                            BreakAlignGroup BreakAlignGroup BreakAlignGroup))
                         (set! heard #f)))))))
           'Align_maker)",
    );
    let scope = vm.make_module(&["lily"]).unwrap();
    let layout = vm.heap_mut().wrap(OutputDef::new(scope));
    vm.heap_mut().root(layout);
    for text in defs {
        let mods = eval(&mut vm, &format!("'{text}"));
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
    Some((vm, layout))
}

/// Makes a `NoteHead` of each note it hears and, as layout's engravers
/// will, sets its object alist in layout's slot at once: `(made . <its
/// number>)`, a list made then and held by nothing else.
#[derive(Clone, Default)]
struct NoteMaker {
    heard: Vec<Value>,
}

impl Work for NoteMaker {
    fn listen(&mut self, _: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        self.heard.push(ev);
        Ok(())
    }
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for ev in std::mem::take(&mut self.heard) {
            let heap = tx.heap();
            let name = heap.intern("NoteHead");
            let ifaces = [heap.intern("note-head-interface")];
            let ifaces = heap.list(&ifaces);
            let g = tx.make_grob(name, Cause::Event(ev), ifaces);
            let heap = tx.heap();
            let key = heap.intern("made");
            let entry = heap.cons(key, Value::fixnum(i64::from(g)).unwrap());
            let objects = heap.list(&[entry]);
            let mut a = tx.t.arena.borrow_mut();
            grobs(&mut a).get_mut(g).unwrap().objects = objects;
        }
        Ok(())
    }
    fn values(&self, out: &mut Vec<Value>) {
        out.extend_from_slice(&self.heard);
    }
}

static NOTE_MAKER: TranslatorClass = TranslatorClass {
    name: "Note_maker",
    kind: Kind::Engraver,
    methods: PROCESS,
    always: false,
    hears: &["note-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(NoteMaker::default())),
    reads: &[],
    writes: &[],
};

/// `music` translated with `Note_maker`, its translation kept rooted.
fn translate(vm: &mut Vm, layout: Value, music: &str) -> Translated {
    let music = eval(vm, music);
    vm.heap_mut().root(music);
    let done = run_translator_with(vm, music, layout, None, &[&NOTE_MAKER])
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    done
}

#[test]
fn layout_keeps_a_grob_of_each_record_by_its_number() {
    let Some((mut vm, layout)) = vm() else { return };
    let done = translate(&mut vm, layout, "(seq (note 0 2) (note 1 2) (note 2 2))");
    let arena = &done.translation.arena;
    // Note_maker runs before the Scheme engraver at each timestep, and
    // filled the slot up to its own record each time.
    let names: Vec<_> = {
        let a = arena.borrow();
        a.records.grobs.iter().map(|g| show(&vm, g.name)).collect()
    };
    assert_eq!(
        names,
        [
            "NoteHead",
            "TextScript",
            "NoteHead",
            "TextScript",
            "NoteHead",
            "TextScript"
        ]
    );
    // Collected, and allocated over: the object alists set in the slot
    // live as long as the translation, which traces them.
    vm.collect();
    eval(&mut vm, "(length (iota 100000))");
    let mut a = arena.borrow_mut();
    let made = a.records.grobs.len();
    let grobs = grobs(&mut a);
    assert_eq!(grobs.len(), made);
    let objects: Vec<_> = (0..made as u32)
        .map(|id| show(&vm, grobs.get(id).unwrap().objects))
        .collect();
    assert_eq!(
        objects,
        [
            "((made . 0))",
            "()",
            "((made . 2))",
            "()",
            "((made . 4))",
            "()"
        ]
    );
}

#[test]
fn a_scheme_engravers_grob_is_found_by_its_handle() {
    let Some((mut vm, layout)) = vm() else { return };
    let done = translate(&mut vm, layout, "(seq (note 0 2) (note 1 2))");
    let made = eval(&mut vm, "(reverse made)");
    let mut ids = Vec::new();
    let mut p = made;
    while let (Some(h), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
        let (arena, id) = grob_of(vm.heap(), h).unwrap();
        assert!(Rc::ptr_eq(&arena, &done.translation.arena));
        let mut a = arena.borrow_mut();
        assert_eq!(show(&vm, a.records.grobs[id as usize].name), "TextScript");
        assert_eq!(show(&vm, grobs(&mut a).get(id).unwrap().objects), "()");
        ids.push(id);
        p = rest;
    }
    assert_eq!(ids, [1, 3]);
    // Not a grob's handle: a note's.
    let note = eval(&mut vm, "(note 0 2)");
    assert!(grob_of(vm.heap(), note).is_none());
}

/// A VM translated with `Glide_maker` over two notes: `(g n)` is the nth
/// grob it made, `seen` the warnings and programming errors since, and
/// the properties the tests set have a `backend-type?`, as
/// `define-grob-properties.scm` gives them, named as `lily.scm`'s
/// `type-name` names `number?`.
fn glided() -> Option<(Vm, Translated)> {
    glided_over("(seq (note 0 2) (note 1 2))")
}

/// [`glided`]'s VM translated over `music`.
fn glided_over(music: &str) -> Option<(Vm, Translated)> {
    glided_with(GLIDE_DEFS, music)
}

/// [`glided`]'s VM with the layout of `defs`, translated over `music`.
fn glided_with(defs: &[&str], music: &str) -> Option<(Vm, Translated)> {
    let (vm, layout) = vm_with(defs)?;
    Some(glided_in(vm, layout, music))
}

/// [`glided`]'s `vm`, whose layout is `layout`, translated over `music`.
fn glided_in(mut vm: Vm, layout: Value, music: &str) -> (Vm, Translated) {
    let done = translate(&mut vm, layout, music);
    eval(
        &mut vm,
        "(define (g n) (list-ref glided n))
         (define seen '())
         (define (see text) (set! seen (append seen (list text))))
         (set! ly:warning (lambda (str . rest) (see (apply format #f str rest))))
         (set! %programming-errored (lambda (texts value) (for-each see texts) value))
         (for-each (lambda (sym) (set-object-property! sym 'backend-type? number?))
                   '(padding staff-padding X-offset Y-offset slur-padding direction))
         (set-object-property! 'minimum-X-extent 'backend-type? pair?)
         (set-object-property! 'text 'backend-type? string?)
         (define-public (type-name predicate) (if (eq? predicate number?) \"number\" \"unknown\"))",
    );
    (vm, done)
}

/// What `text` gives, written, and the warnings it wrote, emptied.
fn run(vm: &mut Vm, text: &str) -> (String, String) {
    let v = eval(vm, text);
    let value = show(vm, v);
    let seen = eval(vm, "(let ((s seen)) (set! seen '()) s)");
    (value, show(vm, seen))
}

/// Each of `calls` and what it gives (nothing for an unspecified value),
/// and its warnings if it wrote any.
fn check(vm: &mut Vm, calls: &[(&str, &str)]) {
    for &(call, want) in calls {
        let (mut value, seen) = run(vm, call);
        if value == "#<unspecified>" {
            value.clear();
        }
        let got = if seen == "()" {
            value
        } else {
            format!("{value} {seen}")
        };
        assert_eq!(got, want, "{call}");
    }
}

#[test]
fn a_property_is_set_then_its_definitions_then_the_default() {
    let Some((mut vm, _done)) = glided() else {
        return;
    };
    check(
        &mut vm,
        &[
            ("(ly:grob-property (g 0) 'direction)", "1"),
            ("(ly:grob-property (g 0) 'padding)", "()"),
            ("(ly:grob-property (g 0) 'padding 'fallback)", "fallback"),
            ("(ly:grob-property (g 1) 'direction 'fallback)", "fallback"),
            ("(ly:grob-set-property! (g 0) 'direction -1)", ""),
            ("(ly:grob-property (g 0) 'direction)", "-1"),
            ("(ly:grob-set-property! (g 0) 'padding 1.25)", ""),
            ("(ly:grob-property (g 0) 'padding)", "1.25"),
            // Set again, in place; set to '(), the default.
            ("(ly:grob-set-property! (g 0) 'padding 0.5)", ""),
            ("(ly:grob-property (g 0) 'padding)", "0.5"),
            ("(ly:grob-set-property! (g 0) 'staff-padding '())", ""),
            (
                "(ly:grob-property (g 0) 'staff-padding 'fallback)",
                "fallback",
            ),
            // The setter of LY_DEFINE_WITH_SETTER.
            ("(set! (ly:grob-property (g 3) 'padding) 2)", ""),
            ("(ly:grob-property (g 3) 'padding)", "2"),
            // backend-type? checked, as LilyPond checks it.
            (
                "(ly:grob-set-property! (g 0) 'padding \"wide\")",
                " (\"the property 'padding' must be of type 'number', ignoring invalid value '\\\"wide\\\"'\")",
            ),
            ("(ly:grob-property (g 0) 'padding)", "0.5"),
            (
                "(ly:grob-set-property! (g 0) 'no-such-property 1)",
                " (\"the property 'no-such-property' does not exist (perhaps a typing error)\")",
            ),
            ("(ly:grob-property (g 0) 'no-such-property)", "()"),
        ],
    );
    let errors = [
        ("(ly:grob-property 1 'padding)", "wrong-type-arg"),
        ("(ly:grob-property (g 0) \"padding\")", "wrong-type-arg"),
        (
            "(ly:grob-set-property! (g 0) \"padding\" 1)",
            "wrong-type-arg",
        ),
        ("(ly:grob-object (note 0 2) 'x)", "wrong-type-arg"),
    ];
    for (call, key) in errors {
        let caught = format!("(catch #t (lambda () {call}) (lambda (key . args) key))");
        assert_eq!(run(&mut vm, &caught).0, key, "{call}");
    }
}

#[test]
fn the_cause_is_the_records() {
    let Some((mut vm, _done)) = glided() else {
        return;
    };
    check(
        &mut vm,
        &[
            ("(ly:stream-event? (ly:grob-property (g 0) 'cause))", "#t"),
            ("(eq? (ly:grob-property (g 1) 'cause) (g 0))", "#t"),
            ("(ly:grob-property (g 2) 'cause)", "()"),
            ("(ly:grob-property (g 2) 'cause 'none)", "none"),
            ("(eq? (ly:grob-property (g 4) 'cause) (g 3))", "#t"),
        ],
    );
}

#[test]
fn a_callback_is_called_once_and_its_value_kept() {
    let Some((mut vm, _done)) = glided() else {
        return;
    };
    check(
        &mut vm,
        &[
            ("(define count 0)", ""),
            (
                "(ly:grob-set-property! (g 0) 'Y-offset (lambda (grob) (set! count (1+ count)) -0.5))",
                "",
            ),
            ("(ly:grob-property (g 0) 'Y-offset)", "-0.5"),
            ("(ly:grob-property (g 0) 'Y-offset)", "-0.5"),
            ("count", "1"),
            // One reading another property.
            ("(ly:grob-set-property! (g 0) 'padding 0.5)", ""),
            (
                "(ly:grob-set-property! (g 0) 'X-offset (lambda (grob) (* 2 (ly:grob-property grob 'padding))))",
                "",
            ),
            ("(ly:grob-property (g 0) 'X-offset)", "1.0"),
            // One giving a procedure, which the next read calls.
            (
                "(ly:grob-set-property! (g 0) 'staff-padding (lambda (grob) (lambda (grob) 2.5)))",
                "",
            ),
            ("(procedure? (ly:grob-property (g 0) 'staff-padding))", "#t"),
            ("(ly:grob-property (g 0) 'staff-padding)", "2.5"),
            // One giving '(): the default, and '() is kept.
            ("(ly:grob-set-property! (g 3) 'padding (lambda (grob) '()))", ""),
            ("(ly:grob-property (g 3) 'padding 'fallback)", "fallback"),
            ("(ly:grob-property (g 3) 'padding)", "()"),
            // Unpure-pure containers: their unpure part.
            (
                "(ly:grob-set-property! (g 0) 'minimum-X-extent
                   (ly:make-unpure-pure-container (lambda (grob) '(0 . 2)) (lambda (grob start end) '(0 . 4))))",
                "",
            ),
            ("(ly:grob-property (g 0) 'minimum-X-extent)", "(0 . 2)"),
            (
                "(ly:grob-set-property! (g 3) 'minimum-X-extent (ly:make-unpure-pure-container '(1 . 3)))",
                "",
            ),
            ("(ly:grob-property (g 3) 'minimum-X-extent)", "(1 . 3)"),
        ],
    );
}

#[test]
fn calculation_in_progress_is_a_cycle() {
    let Some((mut vm, _done)) = glided() else {
        return;
    };
    let cycle = |property: &str, grob: &str| {
        format!(
            "(\"cyclic dependency: calculation-in-progress encountered for {grob}.{property}\")"
        )
    };
    check(
        &mut vm,
        &[
            // A callback that throws leaves the marker, which each read
            // gives with the programming error.
            (
                "(ly:grob-set-property! (g 0) 'slur-padding (lambda (grob) (throw 'thrown)))",
                "",
            ),
            (
                "(catch #t (lambda () (ly:grob-property (g 0) 'slur-padding)) (lambda (key . args) key))",
                "thrown",
            ),
            (
                "(ly:grob-property (g 0) 'slur-padding)",
                &format!(
                    "calculation-in-progress {}",
                    cycle("slur-padding", "TextScript")
                ),
            ),
            ("(ly:grob-set-property! (g 0) 'slur-padding 0.2)", ""),
            ("(ly:grob-property (g 0) 'slur-padding)", "0.2"),
            // A property reading itself, read twice.
            (
                "(ly:grob-set-property! (g 1) 'X-offset (lambda (grob) (ly:grob-property grob 'X-offset)))",
                "",
            ),
            (
                "(ly:grob-property (g 1) 'X-offset)",
                &format!("calculation-in-progress {}", cycle("X-offset", "NoteHead")),
            ),
            (
                "(ly:grob-property (g 1) 'X-offset)",
                &format!("calculation-in-progress {}", cycle("X-offset", "NoteHead")),
            ),
            // Two reading each other.
            (
                "(ly:grob-set-property! (g 2) 'X-offset (lambda (grob) (ly:grob-property grob 'Y-offset)))",
                "",
            ),
            (
                "(ly:grob-set-property! (g 2) 'Y-offset (lambda (grob) (ly:grob-property grob 'X-offset)))",
                "",
            ),
            (
                "(ly:grob-property (g 2) 'X-offset)",
                &format!("calculation-in-progress {}", cycle("X-offset", "NoteHead")),
            ),
            (
                "(ly:grob-property (g 2) 'Y-offset)",
                &format!("calculation-in-progress {}", cycle("Y-offset", "NoteHead")),
            ),
        ],
    );
}

#[test]
fn the_object_alist_holds_grobs_and_their_callbacks() {
    let Some((mut vm, _done)) = glided() else {
        return;
    };
    check(
        &mut vm,
        &[
            ("(ly:grob-object (g 0) 'note-head)", "()"),
            ("(ly:grob-object (g 0) 'note-head 'none)", "none"),
            ("(ly:grob-set-object! (g 0) 'note-head (g 1))", ""),
            ("(eq? (ly:grob-object (g 0) 'note-head) (g 1))", "#t"),
            ("(set! (ly:grob-object (g 0) 'note-head) (g 2))", ""),
            ("(eq? (ly:grob-object (g 0) 'note-head) (g 2))", "#t"),
            // Not a property, nor one: no type is checked.
            ("(ly:grob-property (g 0) 'note-head)", "()"),
            ("(define count 0)", ""),
            (
                "(ly:grob-set-object! (g 0) 'side-support-elements (lambda (grob) (set! count (1+ count)) (list (g 3))))",
                "",
            ),
            (
                "(eq? (car (ly:grob-object (g 0) 'side-support-elements)) (g 3))",
                "#t",
            ),
            (
                "(eq? (car (ly:grob-object (g 0) 'side-support-elements)) (g 3))",
                "#t",
            ),
            ("count", "1"),
        ],
    );
}

#[test]
fn a_dead_grob_takes_nothing_and_keeps_its_cause() {
    let Some((mut vm, done)) = glided() else {
        return;
    };
    check(
        &mut vm,
        &[
            ("(ly:grob-set-property! (g 1) 'padding 1)", ""),
            ("(ly:grob-set-object! (g 1) 'note-head (g 0))", ""),
        ],
    );
    // `suicide`: no definition, nothing set, no objects nor interfaces,
    // the offsets and extents forgotten and the parents kept.
    check(
        &mut vm,
        &[
            ("(ly:grob-set-parent! (g 1) 0 (g 0))", ""),
            ("(ly:grob-translate-axis! (g 1) 2 0)", ""),
            ("(ly:grob-relative-coordinate (g 1) (g 0) 0)", "2.0"),
        ],
    );
    let g1 = eval(&mut vm, "(g 1)");
    suicide(vm.heap_mut(), g1);
    {
        let mut a = done.translation.arena.borrow_mut();
        let record = &a.records.grobs[1];
        assert_eq!(record.definition, Value::NULL);
        assert_eq!(record.props, Value::NULL);
        assert_eq!(record.interfaces, Default::default());
        let grob = grobs(&mut a).get(1).unwrap();
        assert_eq!(grob.objects, Value::NULL);
        assert!(
            grob.dims
                .iter()
                .all(|d| d.offset.is_none() && d.extent.is_none())
        );
    }
    let g0 = eval(&mut vm, "(g 0)");
    assert_eq!(get_parent(vm.heap(), g1, X), Some(g0));
    check(
        &mut vm,
        &[
            ("(ly:grob-set-property! (g 1) 'padding 2)", ""),
            ("(ly:grob-property (g 1) 'padding)", "()"),
            ("(ly:grob-set-object! (g 1) 'note-head (g 0))", ""),
            ("(ly:grob-object (g 1) 'note-head)", "()"),
            ("(eq? (ly:grob-property (g 1) 'cause) (g 0))", "#t"),
        ],
    );
    // `ly:grob-suicide!`: a grob only, a dead one left as it is.
    check(
        &mut vm,
        &[
            ("(ly:grob-suicide! (g 1))", ""),
            ("(eq? (ly:grob-property (g 1) 'cause) (g 0))", "#t"),
            ("(ly:grob-property (g 3) 'direction)", "1"),
            ("(ly:grob-suicide! (g 3))", ""),
            ("(ly:grob-property (g 3) 'direction)", "()"),
            (
                "(catch #t (lambda () (ly:grob-suicide! 1)) (lambda (key . args) key))",
                "wrong-type-arg",
            ),
        ],
    );
}

/// [`glided`]'s VM with `output-lib.scm`'s two callbacks, as the installed
/// file defines them, and `X-padding` a number.
fn chained() -> Option<(Vm, Translated)> {
    chained_over("(seq (note 0 2) (note 1 2))")
}

/// [`glided_over`]'s VM with [`chained`]'s callbacks and
/// `grob::always-Y-extent-from-stencil`.
fn chained_over(music: &str) -> Option<(Vm, Translated)> {
    let (mut vm, done) = glided_over(music)?;
    let text = std::fs::read_to_string(lily_scm().unwrap().join("output-lib.scm")).unwrap();
    for head in [
        "(define-public (grob::compose-function func data)",
        "(define-public (grob::offset-function func data . rest)",
        "(define-public grob::always-Y-extent-from-stencil",
    ] {
        let start = text.find(head).unwrap();
        let mut heap = lily_engine_scheme::Heap::new();
        let form = lily_engine_scheme::read(&mut heap, &text, start, None).unwrap();
        eval(&mut vm, &text[form.start..form.end]);
    }
    eval(
        &mut vm,
        "(set-object-property! 'X-padding 'backend-type? number?)",
    );
    Some((vm, done))
}

#[test]
fn a_chain_composes_a_propertys_data_through_the_image() {
    let Some((mut vm, _done)) = chained() else {
        return;
    };
    let grob = |vm: &mut Vm, n: usize| eval(vm, &format!("(g {n})"));
    // Over the definition's data: (g 0)'s direction, which (g 3), of the
    // same definition, keeps.
    let (g0, g1, g2) = (grob(&mut vm, 0), grob(&mut vm, 1), grob(&mut vm, 2));
    let proc = eval(&mut vm, "(lambda (grob value) (* 3 value))");
    let sym = vm.heap_mut().intern("direction");
    chain_callback(&mut vm, g0, proc, sym).unwrap();
    // Over a procedure, a value set, and nothing.
    check(
        &mut vm,
        &[(
            "(ly:grob-set-property! (g 0) 'padding (lambda (grob) 0.5))",
            "",
        )],
    );
    let proc = eval(&mut vm, "(lambda (grob value) (+ value 1))");
    let sym = vm.heap_mut().intern("padding");
    chain_callback(&mut vm, g0, proc, sym).unwrap();
    let proc = eval(
        &mut vm,
        "(lambda (grob value) (if (null? value) 'none value))",
    );
    let sym = vm.heap_mut().intern("staff-padding");
    chain_callback(&mut vm, g0, proc, sym).unwrap();
    check(
        &mut vm,
        &[
            ("(ly:grob-property (g 0) 'direction)", "3"),
            ("(ly:grob-property (g 3) 'direction)", "1"),
            ("(ly:grob-property (g 0) 'padding)", "1.5"),
            ("(ly:grob-property (g 0) 'staff-padding)", "none"),
        ],
    );
    // An offset added: the procedure alone over nothing, then a sum over
    // the value the first read kept.
    let one = eval(&mut vm, "(lambda (grob) 1)");
    add_offset_callback(&mut vm, g1, one, 0).unwrap();
    check(&mut vm, &[("(ly:grob-property (g 1) 'X-offset)", "1")]);
    let half = eval(&mut vm, "(lambda (grob) 0.5)");
    add_offset_callback(&mut vm, g1, half, 0).unwrap();
    // An offset chained, over a container: its unpure part composed, its
    // pure part too.
    check(
        &mut vm,
        &[(
            "(ly:grob-set-property! (g 1) 'Y-offset
               (ly:make-unpure-pure-container (lambda (grob) 2) (lambda (grob start end) 4)))",
            "",
        )],
    );
    let negate = eval(&mut vm, "(lambda (grob value) (- value))");
    chain_offset_callback(&mut vm, g1, negate, 1).unwrap();
    let data = {
        let (arena, id) = grob_of(vm.heap(), g1).unwrap();
        let sym = vm.heap_mut().intern("Y-offset");
        property_data(vm.heap_mut(), &arena, id, g1, sym)
    };
    assert!(unpure_of(vm.heap(), data).is_some(), "a container");
    let pure_call = eval(&mut vm, "ly:pure-call");
    let (start, end) = (Value::fixnum(0).unwrap(), Value::fixnum(1).unwrap());
    let pure = vm.apply(pure_call, &[data, g1, start, end]).unwrap();
    assert_eq!(show(&vm, pure), "-4");
    check(
        &mut vm,
        &[
            ("(ly:grob-property (g 1) 'X-offset)", "1.5"),
            ("(ly:grob-property (g 1) 'Y-offset)", "-2"),
        ],
    );
    // A dead grob takes no chain.
    {
        let (arena, id) = grob_of(vm.heap(), g2).unwrap();
        arena.borrow_mut().records.grobs[id as usize].definition = Value::NULL;
    }
    let one = eval(&mut vm, "(lambda (grob) 1)");
    add_offset_callback(&mut vm, g2, one, 0).unwrap();
    check(&mut vm, &[("(ly:grob-property (g 2) 'X-offset)", "()")]);
}

#[test]
fn a_chain_needs_the_images_callbacks() {
    let Some((mut vm, _done)) = glided() else {
        return;
    };
    let g0 = eval(&mut vm, "(g 0)");
    let one = eval(&mut vm, "(lambda (grob) 1)");
    let e = add_offset_callback(&mut vm, g0, one, 0).unwrap_err();
    assert_eq!(e.text(vm.heap()), "Unbound variable: grob::offset-function");
}

/// [`GLIDE_DEFS`] with a TextScript whose `meta` has object callbacks and
/// whose definition has a Y-extent of its own.
const MADE_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (interfaces note-head-interface grob-interface)))
        (TextScript (Y-extent . (0 . 1))
                    (meta (name . TextScript) (interfaces text-script-interface grob-interface)
                          (object-callbacks (note-head . made) (stem . made-too)))))))",
    DEFS[1],
    GLIDE_DEFS[2],
];

#[test]
fn the_constructor_sets_the_object_callbacks_and_the_extents() {
    let Some((mut vm, _done)) = glided_with(MADE_DEFS, "(seq (note 0 2))") else {
        return;
    };
    check(
        &mut vm,
        &[
            ("(ly:grob-object (g 0) 'note-head)", "made"),
            ("(ly:grob-object (g 0) 'stem)", "made-too"),
            ("(ly:grob-object (g 1) 'note-head)", "()"),
            // Where the grob has none: the stencil's.
            (
                "(eq? (%grob-property-data (g 1) 'X-extent) ly:grob::stencil-width)",
                "#t",
            ),
            (
                "(ly:unpure-pure-container? (%grob-property-data (g 1) 'Y-extent))",
                "#t",
            ),
            ("(ly:grob-property (g 1) 'X-extent)", "(+inf.0 . -inf.0)"),
            ("(ly:grob-property (g 1) 'Y-extent)", "(+inf.0 . -inf.0)"),
            // Its definition's is kept.
            ("(%grob-property-data (g 0) 'Y-extent)", "(0 . 1)"),
            ("(ly:grob-extent (g 0) (g 0) 1)", "(0.0 . 1.0)"),
        ],
    );
}

#[test]
fn offsets_and_extents_warn_and_check_as_lilypond_does() {
    let Some((mut vm, _done)) = glided() else {
        return;
    };
    eval(
        &mut vm,
        "(set-object-property! 'Y-extent 'backend-type? pair?)",
    );
    let thrown = |call: &str| {
        format!(
            "(catch #t (lambda () {call}) (lambda (key subr message args rest) (list key subr args)))"
        )
    };
    check(
        &mut vm,
        &[
            ("(ly:grob-set-parent! (g 1) 1 (g 0))", ""),
            ("(ly:grob-set-property! (g 1) 'Y-offset +inf.0)", ""),
            ("(ly:grob-set-property! (g 1) 'Y-extent '(0 . 1))", ""),
            (
                "(ly:grob-extent (g 1) (g 0) 1)",
                "(0.0 . 1.0) (\"ignored infinite Y-offset\")",
            ),
            (
                "(ly:grob-translate-axis! (g 2) +nan.0 0)",
                " (\"Infinity or NaN encountered\")",
            ),
            ("(ly:grob-relative-coordinate (g 2) (g 2) 0)", "0.0"),
            (
                &thrown("(ly:grob-extent (g 0) (g 1) 1)"),
                "(misc-error \"ly:grob-extent\" (\"NoteHead\" \"TextScript\" \"Y\"))",
            ),
            (
                &thrown("(ly:grob-relative-coordinate (g 1) (g 0) 0)"),
                "(misc-error \"ly:grob-relative-coordinate\" (\"TextScript\" \"NoteHead\" \"X\"))",
            ),
            (
                "(catch #t (lambda () (ly:grob-parent 'x 0)) (lambda (key . args) key))",
                "wrong-type-arg",
            ),
            (
                "(catch #t (lambda () (ly:grob-set-parent! (g 2) 2 (g 0))) (lambda (key . args) key))",
                "wrong-type-arg",
            ),
            (
                "(catch #t (lambda () (ly:grob-common-refpoint (g 2) 'x 0)) (lambda (key . args) key))",
                "wrong-type-arg",
            ),
            ("(set! (ly:grob-parent (g 2) 0) (g 0))", ""),
            ("(eq? (ly:grob-parent (g 2) 0) (g 0))", "#t"),
            ("(ly:grob-parent (g 2) 1)", "()"),
            ("(ly:grob-parent (g 2) 1 'none)", "none"),
            ("(ly:grob-common-refpoint (g 3) (g 4) 0)", "#f"),
            ("(ly:grob::x-parent-positioning (g 4))", "0.0"),
            ("(ly:grob::stencil-height (g 4))", "(+inf.0 . -inf.0)"),
            (
                "(ly:grob::pure-stencil-height (g 4) 0 1)",
                "(+inf.0 . -inf.0)",
            ),
        ],
    );
}

#[test]
fn rust_reaches_parents_offsets_and_extents_through_a_host() {
    let Some((mut vm, _done)) = glided() else {
        return;
    };
    let g: Vec<Value> = (0..5).map(|n| eval(&mut vm, &format!("(g {n})"))).collect();
    set_parent(vm.heap(), g[1], g[0], X);
    set_parent(vm.heap(), g[2], g[1], X);
    assert_eq!(get_parent(vm.heap(), g[2], X), Some(g[1]));
    assert_eq!(get_parent(vm.heap(), g[2], Y), None);
    eval(
        &mut vm,
        "(ly:grob-set-property! (g 1) 'X-offset (lambda (grob) 1.5))
         (set-object-property! 'X-extent 'backend-type? pair?)
         (ly:grob-set-property! (g 2) 'X-extent '(0 . 1))",
    );
    translate_axis(vm.heap(), g[2], 0.25, X).unwrap();
    assert_eq!(
        translate_axis(vm.heap(), g[2], f64::NAN, X),
        Err(INFINITE_TRANSLATION)
    );
    assert_eq!(relative_coordinate(&mut vm, g[2], g[0], X).unwrap(), 1.75);
    assert_eq!(
        relative_coordinate(&mut vm, g[2], Value::NULL, X).unwrap(),
        1.75
    );
    assert_eq!(
        extent(&mut vm, g[2], g[0], X).unwrap(),
        Interval {
            left: 1.75,
            right: 2.75
        }
    );
    assert_eq!(common_refpoint(vm.heap(), g[2], g[0], X), Some(g[0]));
    assert_eq!(common_refpoint(vm.heap(), g[2], g[1], X), Some(g[1]));
    assert_eq!(common_refpoint(vm.heap(), g[2], g[3], X), None);
    // The offset computed is kept, and the property gives it up.
    check(&mut vm, &[("(ly:grob-property (g 1) 'X-offset)", "()")]);
}

#[test]
fn a_pure_child_translation_is_0_without_elements() {
    let Some((mut vm, _done)) = glided() else {
        return;
    };
    // Elements that are no grob array are none (`extract_grob_set`): no
    // translations (MS3 step 28's, tests/layout.rs).
    eval(
        &mut vm,
        "(ly:grob-set-object! (g 0) 'elements (list (g 1) (g 2)))",
    );
    let (g0, g1) = (eval(&mut vm, "(g 0)"), eval(&mut vm, "(g 1)"));
    assert_eq!(
        get_pure_child_y_translation(&mut vm, g0, g1, 0, 1).unwrap(),
        0.0
    );
}

/// `tools/` of the engine.
fn tools() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools")
}

#[test]
fn the_probes_calls_give_what_lilypond_gives() {
    let Some((mut vm, _done)) = chained_over("(seq (note 0 2) (note 1 2) (note 2 2))") else {
        return;
    };
    let version = Command::new("lilypond").arg("--version").output();
    if !version.is_ok_and(|v| String::from_utf8_lossy(&v.stdout).contains("GNU LilyPond 2.26.0")) {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the probe is unverified against it");
        return;
    }
    // grob-calls.ly's records, by their calls.
    let dir = std::env::temp_dir().join(format!("lily-engine-grob-calls-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let capture = std::fs::canonicalize(tools().join("compare/grob-calls.ly")).unwrap();
    let out = Command::new("lilypond")
        .args(["-s", "-o", "grob-calls"])
        .arg(capture)
        .current_dir(&dir)
        .output()
        .unwrap();
    let lilypond = std::fs::read_to_string(dir.join("grob-calls.txt"));
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lilypond: Vec<(&str, &str)> = lilypond
        .as_deref()
        .unwrap()
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .collect();
    // The fixture's grobs, glided's nine heard as the fixture's first are,
    // their types the fixture's; the two sections such grobs answer as the
    // fixture's do (the others read the fixture's definitions, causes and
    // origins, which tools/compare/test/grobs.test.mjs's compiles have);
    // Fingering's Y-extent as define-grobs.scm gives it; the properties
    // set with their backend-type?, and the stencils' heads.
    eval(
        &mut vm,
        "(define-public X 0)
         (define-public Y 1)
         (define-public probe-grobs-sections '(chains parents))
         (define-public grob-calls-heard
           (map (lambda (n type) (cons (g n) type))
                (iota 9)
                '(Fingering StrokeFinger StringNumber StrokeFinger Fingering
                  StringNumber Fingering StringNumber StrokeFinger)))
         (define-public all-grob-descriptions
           `((Fingering (Y-extent . ,grob::always-Y-extent-from-stencil))))
         (for-each (lambda (sym) (set-object-property! sym 'backend-type? pair?))
                   '(X-extent Y-extent minimum-Y-extent))
         (set-object-property! 'stencil 'backend-type? ly:stencil?)
         (set-object-property! 'positioning-done 'backend-type? boolean?)
         (for-each ly:register-stencil-expression '(round-filled-box draw-line circle))",
    );
    let probe = std::fs::read_to_string(tools().join("bench/probe-grobs.scm")).unwrap();
    if let Err(e) = vm.eval_text(&probe, None) {
        panic!("the probe fails: {}", e.text(vm.heap()));
    }
    let output = vm.take_output();
    let records: Vec<(&str, &str)> = output
        .lines()
        .map(|l| l.split_once('\t').unwrap())
        .collect();
    assert_eq!(
        records.len(),
        75,
        "the engine ran every call of the two sections"
    );
    // Each record against the capture's next record of the same call: a
    // call made twice may give two values. LilyPond's grob-calls-count
    // after the translation is 1, the callbacks section's one call, which
    // is not run here.
    let mut next = 0;
    let mut failures = Vec::new();
    for (call, value) in &records {
        let found = lilypond[next..].iter().position(|(c, _)| c == call);
        let want = found.map(|i| lilypond[next + i].1);
        let want = if *call == "grob-calls-count" {
            want.and(Some("0"))
        } else {
            want
        };
        if want != Some(*value) {
            failures.push(format!("{call}\n  engine   {value}\n  lilypond {want:?}"));
        }
        next += found.map_or(0, |i| i + 1);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The forms of the installed `file` that begin with `heads`, evaluated.
fn load_forms(vm: &mut Vm, file: &str, heads: &[&str]) {
    let text = std::fs::read_to_string(lily_scm().unwrap().join(file)).unwrap();
    for head in heads {
        let start = text.find(head).unwrap_or_else(|| panic!("{file}: {head}"));
        let mut heap = lily_engine_scheme::Heap::new();
        let form = lily_engine_scheme::read(&mut heap, &text, start, None).unwrap();
        eval(vm, &text[form.start..form.end]);
    }
}

/// [`glided`]'s VM over a note at `/s/a b.ly`'s line 3, character 4 and
/// column 6 (`loc`) and one without an origin, with the installed
/// `color.scm`, `stencil-with-color` and `stencil-whiteout` of
/// `stencil.scm` and what they call, `ly:in-event-class?`, the stencil
/// heads they make, and the layout's `line-thickness` 0.1.
fn printed() -> Option<(Vm, Translated)> {
    let (mut vm, layout) = vm_with(GLIDE_DEFS)?;
    let name: Rc<str> = Rc::from("/s/a b.ly");
    let loc = vm.heap_mut().wrap(Input::new(0, 10, 12, name, [3, 4, 6]));
    vm.define("loc", loc);
    let scope = vm.heap().foreign::<OutputDef>(layout).unwrap().scope;
    let thickness = vm.heap_mut().flonum(0.1);
    vm.module_define(scope, "line-thickness", thickness);
    let (mut vm, done) = glided_in(
        vm,
        layout,
        "(seq (let ((n (note 0 2))) (ly:music-set-property! n 'origin loc) n) (note 1 2))",
    );
    eval(&mut vm, "(use-modules (srfi srfi-1))");
    let color = lily_scm().unwrap().join("color.scm");
    let text = std::fs::read_to_string(&color).unwrap();
    vm.eval_text(&text, color.to_str())
        .unwrap_or_else(|e| panic!("color.scm: {}", e.text(vm.heap())));
    eval(&mut vm, "(define-public X 0) (define-public Y 1)");
    load_forms(
        &mut vm,
        "lily-library.scm",
        &["(define-public (interval-widen"],
    );
    load_forms(
        &mut vm,
        "stencil.scm",
        &[
            "(define-public (stencil-with-color",
            "(define*-public (stencil-whiteout-box",
            "(define*-public (stencil-whiteout ",
        ],
    );
    load_forms(
        &mut vm,
        "define-event-classes.scm",
        &["(define-public (ly:in-event-class?"],
    );
    eval(
        &mut vm,
        "(for-each ly:register-stencil-expression '(round-filled-box color))
         (define box (ly:round-filled-box '(0 . 1) '(0 . 2) 0))",
    );
    Some((vm, done))
}

/// The grob of the first `grob-cause` in `expr`.
fn linked(vm: &Vm, expr: Value) -> Option<Value> {
    let heap = vm.heap();
    let (head, rest) = (heap.car(expr)?, heap.cdr(expr)?);
    if heap.name(head) == Some("grob-cause") {
        return heap.car(rest);
    }
    linked(vm, head).or_else(|| linked(vm, rest))
}

/// The print stencil of [`glided`]'s `(g n)`: its expression, written
/// with the grob as `<grob>`, which must be `(g n)`, and its extents; or
/// the error's text.
fn print(vm: &mut Vm, n: usize) -> String {
    let grob = eval(vm, &format!("(g {n})"));
    match get_print_stencil(vm, grob) {
        Ok(s) => {
            if let Some(g) = linked(vm, s.expr) {
                assert_eq!(g, grob, "the stencil of (g {n}) links its grob");
            }
            let expr = show(vm, s.expr).replace(&show(vm, grob), "<grob>");
            let (x, y) = (s.extent(X), s.extent(Y));
            format!("{expr} ({} {}) ({} {})", x.left, x.right, y.left, y.right)
        }
        Err(e) => e.text(vm.heap()),
    }
}

#[test]
fn the_print_stencil_wraps_the_stencil_as_lilypond_does() {
    let Some((mut vm, _done)) = printed() else {
        return;
    };
    let set = |vm: &mut Vm, n: usize, sym: &str, value: &str| {
        eval(
            vm,
            &format!("(%grob-set-property-data! (g {n}) '{sym} {value})"),
        );
    };
    let boxed = "(round-filled-box -0.0 1.0 -0.0 2.0 0.0)";
    // No stencil: nothing.
    assert_eq!(print(&mut vm, 0), "() (inf -inf) (inf -inf)");
    // The stencil, from its callback, linked to its grob.
    set(&mut vm, 0, "stencil", "(lambda (grob) box)");
    assert_eq!(
        print(&mut vm, 0),
        format!("(grob-cause <grob> {boxed}) (0 1) (0 2)")
    );
    // Rotated about its centre, in color, with its attributes outside.
    set(&mut vm, 0, "rotation", "'(90 0 0)");
    set(&mut vm, 0, "color", "\"red\"");
    set(&mut vm, 0, "output-attributes", "'((id . \"x\"))");
    assert_eq!(
        print(&mut vm, 0),
        format!(
            "(output-attributes ((id . \"x\")) (color (1 0 0 1.0) (rotate-stencil (90.0 (0.5 . 1.0)) (grob-cause <grob> {boxed})))) (-0.5 1.5) (0.5 1.5)"
        )
    );
    // A color's list without its alpha; the whiteout inside the link, in
    // white for a color that is none, widened by its thickness of
    // line-thickness.
    set(&mut vm, 3, "stencil", "box");
    set(&mut vm, 3, "color", "'(0 0 1)");
    set(&mut vm, 3, "whiteout", "2");
    set(&mut vm, 3, "whiteout-color", "#t");
    assert_eq!(
        print(&mut vm, 3),
        format!(
            "(color (0.0 0.0 1.0 1.0) (grob-cause <grob> (combine-stencil (color (1 1 1 1.0) (round-filled-box 0.2 1.2 0.2 2.2 0.0)) {boxed}))) (-0.2 1.2) (-0.2 2.2)"
        )
    );
    // Transparent: the extents only, no whiteout, no link.
    set(&mut vm, 3, "transparent", "#t");
    assert_eq!(
        print(&mut vm, 3),
        "(color (0.0 0.0 1.0 1.0) ()) (0 1) (0 2)"
    );
    // Skylines wait for part 3.
    set(&mut vm, 1, "show-vertical-skylines", "#t");
    assert_eq!(
        print(&mut vm, 1),
        "In procedure Grob::get_print_stencil: skylines are not in the engine before MS3's part 3"
    );
}

#[test]
fn a_grobs_warnings_are_given_at_its_origin() {
    let Some((mut vm, _done)) = printed() else {
        return;
    };
    let g: Vec<Value> = (0..6).map(|n| eval(&mut vm, &format!("(g {n})"))).collect();
    let loc = eval(&mut vm, "loc");
    // The TextScript of the located note and the NoteHead it caused are at
    // the note; the NoteHead of no cause, and the second note's, nowhere.
    assert_eq!(origin(vm.heap_mut(), g[0]), Some(loc));
    assert_eq!(origin(vm.heap_mut(), g[1]), Some(loc));
    assert_eq!(origin(vm.heap_mut(), g[2]), None);
    assert_eq!(origin(vm.heap_mut(), g[3]), None);
    eval(
        &mut vm,
        "(define (kept) (let ((d (car %diagnostics))) (set-car! %diagnostics '()) (map (lambda (v) (list (vector-ref v 0) (vector-ref v 1) (vector-ref v 2))) (reverse d))))",
    );
    // A primitive's, Rust's, and a cycle's, at the origin.
    eval(
        &mut vm,
        "(ly:grob-translate-axis! (g 1) +nan.0 0)
         (%grob-set-property-data! (g 0) 'padding (lambda (grob) (ly:grob-property grob 'padding)))
         (ly:grob-property (g 0) 'padding)",
    );
    warning(&mut vm, g[1], "a ~a warning").unwrap();
    programming_error(&mut vm, g[0], "an error").unwrap();
    let kept = eval(&mut vm, "(kept)");
    assert_eq!(
        show(&vm, kept),
        "((programming-error #<location /s/a b.ly:3:7> \"Infinity or NaN encountered\") \
         (programming-error #<location /s/a b.ly:3:7> \"cyclic dependency: calculation-in-progress encountered for TextScript.padding\") \
         (warning #<location /s/a b.ly:3:7> \"a ~a warning\") \
         (programming-error #<location /s/a b.ly:3:7> \"an error\"))"
    );
    // Without an origin, as LilyPond's own: `ly:warning`, which glided
    // keeps in `seen`, and `ly:programming-error`.
    check(
        &mut vm,
        &[(
            "(ly:grob-translate-axis! (g 3) +inf.0 1)",
            " (\"Infinity or NaN encountered\")",
        )],
    );
    warning(&mut vm, g[2], "a ~a warning").unwrap();
    programming_error(&mut vm, g[3], "an error").unwrap();
    let kept = eval(&mut vm, "(kept)");
    assert_eq!(show(&vm, kept), "((programming-error #f \"an error\"))");
    let seen = eval(&mut vm, "seen");
    assert_eq!(show(&vm, seen), "(\"a ~a warning\")");
}

#[test]
fn the_writer_links_an_event_causes_origin() {
    let Some((mut vm, _done)) = printed() else {
        return;
    };
    let g: Vec<Value> = (0..6).map(|n| eval(&mut vm, &format!("(g {n})"))).collect();
    // `output-svg.scm`'s `grob-cause`'s link, the file's name absolute and
    // percent-encoded, its column from 1, under each `point-and-click`.
    let link = Some(String::from("textedit:///s/a%20b.ly:3:4:7"));
    let mut links = |option: &str| -> [Option<String>; 2] {
        eval(
            &mut vm,
            &format!("(ly:set-option 'point-and-click {option})"),
        );
        [0, 1].map(|n| cause_link(&mut vm, g[n]).unwrap())
    };
    assert_eq!(links("#t"), [link.clone(), None]);
    assert_eq!(links("'note-event"), [link.clone(), None]);
    assert_eq!(links("'(rest-event note-event)"), [link.clone(), None]);
    assert_eq!(links("'rest-event"), [None, None]);
    assert_eq!(links("#f"), [None, None]);
    eval(&mut vm, "(ly:set-option 'point-and-click #t)");
    // A grob caused by a grob has none, nor one of an event without an
    // origin, nor what is no grob; Scheme reads the same link.
    assert_eq!(cause_link(&mut vm, g[3]).unwrap(), None);
    let event = eval(&mut vm, "(ly:grob-property (g 0) 'cause)");
    assert_eq!(cause_link(&mut vm, event).unwrap(), None);
    assert_eq!(cause_link(&mut vm, Value::FALSE).unwrap(), None);
    let read = eval(&mut vm, "(%grob-cause-link (g 0))");
    assert_eq!(vm.heap().str(read).map(|s| String::from(s.as_str())), link);
}

/// [`GLIDE_DEFS`] whose definitions name their classes, as
/// `define-grobs.scm`'s `completize-grob-entry` gives them: a `NoteHead`
/// is an `Item`, a `TextScript` either class.
const CLASSED_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface)))
        (TextScript (direction . 1)
                    (meta (name . TextScript) (classes Item Spanner)
                          (interfaces grob-interface text-script-interface))))))",
    DEFS[1],
    GLIDE_DEFS[2],
];

#[test]
fn a_grob_has_its_definitions_interfaces_and_its_classs() {
    let Some((mut vm, _done)) = glided_with(CLASSED_DEFS, "(seq (note 0 2) (note 1 2))") else {
        return;
    };
    let g: Vec<Value> = (0..6).map(|n| eval(&mut vm, &format!("(g {n})"))).collect();
    // A NoteHead takes its one class and the interface Item's constructor
    // adds; a TextScript, of two, none until an engraver gives it one.
    let heap = vm.heap_mut();
    assert_eq!(class(heap, g[1]), Some(Class::Item));
    for (name, has) in [
        ("grob-interface", true),
        ("note-head-interface", true),
        ("item-interface", true),
        ("spanner-interface", false),
        ("text-script-interface", false),
    ] {
        assert_eq!(has_interface(heap, g[1], name), has, "{name}");
    }
    assert_eq!(class(heap, g[0]), None);
    assert!(has_interface(heap, g[0], "text-script-interface"));
    assert!(!has_interface(heap, g[0], "item-interface"));
    set_class(heap, g[0], Class::Spanner);
    assert_eq!(class(heap, g[0]), Some(Class::Spanner));
    assert!(has_interface(heap, g[0], "spanner-interface"));
    assert!(!is_item(heap, g[0]));
    // A class given is kept: its definition's is not looked for.
    set_class(heap, g[5], Class::System);
    assert_eq!(class(heap, g[5]), Some(Class::System));
    assert!(has_interface(heap, g[5], "system-interface"));
    assert!(has_interface(heap, g[5], "spanner-interface"));
    assert!(!has_interface(heap, g[5], "item-interface"));
    set_class(heap, g[2], Class::PaperColumn);
    assert!(is_item(heap, g[2]));
    assert!(has_interface(heap, g[2], "paper-column-interface"));
    assert!(has_interface(heap, g[2], "item-interface"));
    // What is no grob has none.
    let note = eval(&mut vm, "(note 0 2)");
    assert!(!has_interface(vm.heap_mut(), note, "grob-interface"));
    assert_eq!(class(vm.heap_mut(), note), None);
    // A dead grob has no interface, and is named by its class.
    let thrown = |call: &str| {
        format!(
            "(catch #t (lambda () {call}) (lambda (key subr message args rest) (list key subr args)))"
        )
    };
    check(
        &mut vm,
        &[
            ("(ly:grob-suicide! (g 4))", ""),
            (
                &thrown("(ly:grob-extent (g 3) (g 4) 0)"),
                "(misc-error \"ly:grob-extent\" (\"dead Item\" \"TextScript\" \"X\"))",
            ),
        ],
    );
    let g4 = eval(&mut vm, "(g 4)");
    assert!(!has_interface(vm.heap_mut(), g4, "item-interface"));
    assert!(!has_interface(vm.heap_mut(), g4, "grob-interface"));
    assert!(is_item(vm.heap_mut(), g4));
}

#[test]
fn ly_add_interface_describes_an_interface_layout_does_not_check() {
    let Some((mut vm, _done)) = glided_with(CLASSED_DEFS, "(seq (note 0 2))") else {
        return;
    };
    check(
        &mut vm,
        &[
            (
                "(ly:add-interface 'probe-interface \"A probe.\" '(padding))",
                "",
            ),
            (
                "(hashq-ref (ly:all-grob-interfaces) 'probe-interface)",
                "(probe-interface \"A probe.\" (padding))",
            ),
            // A property no interface of the grob's describes is set and
            // read: the check is `check-internal-types`'.
            ("(ly:grob-set-property! (g 1) 'staff-padding 2)", ""),
            ("(ly:grob-property (g 1) 'staff-padding)", "2"),
        ],
    );
    let g1 = eval(&mut vm, "(g 1)");
    assert!(!has_interface(vm.heap_mut(), g1, "probe-interface"));
    // A grob layout had not reached keeps its class when it dies.
    let g2 = eval(&mut vm, "(g 2)");
    suicide(vm.heap_mut(), g2);
    assert_eq!(class(vm.heap_mut(), g2), Some(Class::Item));
    assert!(!has_interface(vm.heap_mut(), g2, "item-interface"));
}

#[test]
fn an_item_has_a_break_direction_and_a_column() {
    let Some((mut vm, _done)) = glided_with(CLASSED_DEFS, "(seq (note 0 2) (note 1 2))") else {
        return;
    };
    let g: Vec<Value> = (0..6).map(|n| eval(&mut vm, &format!("(g {n})"))).collect();
    let thrown = |call: &str| {
        format!(
            "(catch #t (lambda () {call}) (lambda (key subr message args rest) (list key subr message args)))"
        )
    };
    check(
        &mut vm,
        &[
            ("(ly:item? (g 1))", "#t"),
            ("(ly:item? (g 0))", "#f"),
            ("(ly:item? 'x)", "#f"),
            ("(ly:item-break-dir (g 1))", "0"),
            ("(ly:item-get-column (g 1))", "()"),
            (
                &thrown("(ly:item-break-dir (g 0))"),
                "(wrong-type-arg \"ly:item-break-dir\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (1 \"Item\" #<Grob TextScript >))",
            ),
            (
                &thrown("(ly:item-get-column 3)"),
                "(wrong-type-arg \"ly:item-get-column\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (1 \"Item\" 3))",
            ),
        ],
    );
    // A column is its own; an item's is its parent's on X while that is an
    // item: (g 4) under (g 1) under the column (g 2), not under (g 0), of
    // no class.
    set_class(vm.heap_mut(), g[2], Class::PaperColumn);
    set_parent(vm.heap(), g[1], g[2], X);
    set_parent(vm.heap(), g[4], g[1], X);
    set_parent(vm.heap(), g[5], g[0], X);
    set_parent(vm.heap(), g[3], g[2], Y);
    let heap = vm.heap_mut();
    assert_eq!(get_column(heap, g[2]), Some(g[2]));
    assert_eq!(get_column(heap, g[1]), Some(g[2]));
    assert_eq!(get_column(heap, g[4]), Some(g[2]));
    assert_eq!(get_column(heap, g[5]), None);
    assert_eq!(get_column(heap, g[3]), None);
    check(
        &mut vm,
        &[
            ("(eq? (ly:item-get-column (g 4)) (g 2))", "#t"),
            ("(eq? (ly:item-get-column (g 2)) (g 2))", "#t"),
            ("(ly:item? (g 2))", "#t"),
        ],
    );
    // The slots of the broken copies, as breaking will fill them: (g 1)
    // broken into (g 4) at the end of a line and (g 5) at the beginning of
    // the next. Collected and allocated over, they are kept.
    {
        let (arena, id) = grob_of(vm.heap(), g[1]).unwrap();
        let mut a = arena.borrow_mut();
        let grobs = grobs(&mut a);
        grobs.get_mut(id).unwrap().broken = [Some(g[4]), Some(g[5])];
        for copy in [4, 5] {
            let (_, copy) = grob_of(vm.heap(), g[copy]).unwrap();
            grobs.get_mut(copy).unwrap().original = Some(g[1]);
        }
    }
    vm.collect();
    eval(&mut vm, "(length (iota 100000))");
    let heap = vm.heap();
    assert!(is_broken(heap, g[1]));
    assert!(!is_broken(heap, g[4]));
    assert_eq!(break_status_dir(heap, g[1]), 0);
    assert_eq!(break_status_dir(heap, g[4]), -1);
    assert_eq!(break_status_dir(heap, g[5]), 1);
    assert_eq!(find_prebroken_piece(heap, g[1], -1), Some(g[4]));
    assert_eq!(find_prebroken_piece(heap, g[1], 1), Some(g[5]));
    assert_eq!(find_prebroken_piece(heap, g[1], 0), Some(g[1]));
    assert_eq!(find_prebroken_piece(heap, g[2], 1), None);
    check(
        &mut vm,
        &[
            ("(ly:item-break-dir (g 4))", "-1"),
            ("(ly:item-break-dir (g 5))", "1"),
        ],
    );
}

/// `call`, caught: the key and the arguments of what it throws.
fn caught(call: &str) -> String {
    format!(
        "(catch #t (lambda () {call}) (lambda (key subr message args rest) (list key subr message args rest)))"
    )
}

#[test]
fn a_grob_array_holds_its_grobs_as_lilypond_gives_them() {
    let Some((mut vm, _done)) = glided_with(CLASSED_DEFS, "(seq (note 0 2) (note 1 2))") else {
        return;
    };
    eval(
        &mut vm,
        "(define arr (ly:grob-list->grob-array (list (g 1) (g 0))))",
    );
    // What LilyPond 2.26.0 gives for each call, a NoteHead and a
    // TextScript in place of its NoteHead and Stem.
    let wrong = "\"Wrong type argument in position ~A (expecting ~A): ~S\"";
    check(
        &mut vm,
        &[
            ("arr", "#<Grob_array#<Grob NoteHead > #<Grob TextScript > >"),
            ("(ly:grob-list->grob-array '())", "#<Grob_array>"),
            ("(ly:grob-array? arr)", "#t"),
            ("(ly:grob-array? (g 1))", "#f"),
            ("(ly:grob-array-length arr)", "2"),
            ("(eq? (ly:grob-array-ref arr 1) (g 0))", "#t"),
            (
                "(ly:grob-array->list arr)",
                "(#<Grob NoteHead > #<Grob TextScript >)",
            ),
            (
                &caught("(ly:grob-array-length 3)"),
                &format!(
                    "(wrong-type-arg \"ly:grob-array-length\" {wrong} (1 \"Grob_array\" 3) (3))"
                ),
            ),
            (
                &caught("(ly:grob-array-ref arr 2)"),
                "(out-of-range #f \"Value out of range: ~S\" (2) (2))",
            ),
            (
                &caught("(ly:grob-array-ref arr -1)"),
                "(out-of-range #f \"Value out of range ~S to< ~S: ~S\" (0 4294967295 -1) (-1))",
            ),
            (
                &caught("(ly:grob-array-ref arr 1.0)"),
                "(wrong-type-arg #f \"Wrong type (expecting ~A): ~S\" (\"exact integer\" 1.0) (1.0))",
            ),
            (
                &caught("(ly:grob-array-ref arr 5000000000)"),
                "(out-of-range #f \"Value out of range ~S to< ~S: ~S\" (0 4294967295 5000000000) (5000000000))",
            ),
            (
                &caught("(ly:grob-array-ref arr 'x)"),
                &format!("(wrong-type-arg \"ly:grob-array-ref\" {wrong} (2 \"integer\" x) (x))"),
            ),
            (
                &caught("(ly:grob-array-ref 3 0)"),
                &format!("(wrong-type-arg \"ly:grob-array-ref\" {wrong} (1 \"Grob_array\" 3) (3))"),
            ),
            // Filtered in order: the TextScript, of no class, is no item.
            (
                "(ly:grob-array-filter ly:item? arr)",
                "#<Grob_array#<Grob NoteHead > >",
            ),
            (
                &caught("(ly:grob-array-filter 3 arr)"),
                &format!(
                    "(wrong-type-arg \"ly:grob-array-filter\" {wrong} (1 \"procedure\" 3) (3))"
                ),
            ),
            (
                &caught("(ly:grob-array-filter ly:item? 3)"),
                &format!(
                    "(wrong-type-arg \"ly:grob-array-filter\" {wrong} (2 \"Grob_array\" 3) (3))"
                ),
            ),
            (
                "(ly:grob-list->grob-array (list 1 (g 1)))",
                "#<Grob_array#<Grob NoteHead > > (\"ly:grob-list->grob-array encountered a non-grob object\")",
            ),
            (
                "(ly:grob-list->grob-array (cons (g 1) 3))",
                "#<Grob_array#<Grob NoteHead > > (\"ly:grob-list->grob-array expected a list\")",
            ),
            (
                "(ly:grob-list->grob-array 3)",
                "#<Grob_array> (\"ly:grob-list->grob-array expected a list\")",
            ),
            (
                "(equal? arr (ly:grob-list->grob-array (list (g 1) (g 0))))",
                "#f",
            ),
            ("(equal? arr arr)", "#t"),
        ],
    );
    // A filtered array is ordered as its source; an unordered one keeps
    // each grob once, where it first is (`uniquify`).
    let arr = eval(&mut vm, "arr");
    let heap = vm.heap_mut();
    heap.root(arr);
    heap.foreign_mut::<GrobArray>(arr).unwrap().ordered = false;
    let kept = eval(&mut vm, "(ly:grob-array-filter (lambda (g) #t) arr)");
    assert!(!vm.heap().foreign::<GrobArray>(kept).unwrap().ordered);
    let g: Vec<Value> = (0..3).map(|n| eval(&mut vm, &format!("(g {n})"))).collect();
    let mut dup = GrobArray {
        grobs: vec![g[2], g[0], g[2], g[1], g[0]],
        ordered: false,
    };
    dup.remove_duplicates(vm.heap());
    assert_eq!(dup.grobs, [g[2], g[0], g[1]]);
    // An array in an object slot keeps its grobs through a collection.
    eval(
        &mut vm,
        "(ly:grob-set-object! (g 2) 'kept (ly:grob-list->grob-array (list (g 3) (g 4))))",
    );
    vm.collect();
    eval(&mut vm, "(length (iota 100000))");
    check(
        &mut vm,
        &[(
            "(ly:grob-array->list (ly:grob-object (g 2) 'kept))",
            "(#<Grob TextScript > #<Grob NoteHead >)",
        )],
    );
}

#[test]
fn a_pointer_group_adds_to_the_grob_array_in_an_object_slot() {
    let Some((mut vm, _done)) = glided_with(CLASSED_DEFS, "(seq (note 0 2) (note 1 2))") else {
        return;
    };
    let wrong = "\"Wrong type argument in position ~A (expecting ~A): ~S\"";
    let add = "ly:pointer-group-interface::add-grob";
    check(
        &mut vm,
        &[
            // Made the first time, then added to.
            (
                "(ly:pointer-group-interface::add-grob (g 1) 'probe-set (g 0))",
                "",
            ),
            (
                "(ly:grob-object (g 1) 'probe-set)",
                "#<Grob_array#<Grob TextScript > >",
            ),
            (
                "(ly:pointer-group-interface::add-grob (g 1) 'probe-set (g 1))",
                "",
            ),
            (
                "(ly:grob-array->list (ly:grob-object (g 1) 'probe-set))",
                "(#<Grob TextScript > #<Grob NoteHead >)",
            ),
            (
                &caught("(ly:pointer-group-interface::add-grob 3 'x (g 1))"),
                &format!("(wrong-type-arg \"{add}\" {wrong} (1 \"Grob\" 3) (3))"),
            ),
            (
                &caught("(ly:pointer-group-interface::add-grob (g 1) \"x\" (g 1))"),
                &format!("(wrong-type-arg \"{add}\" {wrong} (2 \"symbol\" \"x\") (\"x\"))"),
            ),
            (
                &caught("(ly:pointer-group-interface::add-grob (g 1) 'x 3)"),
                &format!("(wrong-type-arg \"{add}\" {wrong} (3 \"Grob\" 3) (3))"),
            ),
            // The slot's callback is called once, and its array added to;
            // what is no array is replaced.
            (
                "(ly:grob-set-object! (g 1) 'probe-cb
                   (lambda (gr) (see \"called\") (ly:grob-list->grob-array (list (g 0)))))",
                "",
            ),
            (
                "(ly:pointer-group-interface::add-grob (g 1) 'probe-cb (g 1))",
                " (\"called\")",
            ),
            (
                "(ly:grob-array-length (ly:grob-object (g 1) 'probe-cb))",
                "2",
            ),
            ("(ly:grob-set-object! (g 1) 'probe-num 3)", ""),
            (
                "(ly:pointer-group-interface::add-grob (g 1) 'probe-num (g 1))",
                "",
            ),
            (
                "(ly:grob-array-length (ly:grob-object (g 1) 'probe-num))",
                "1",
            ),
            // A dead grob takes no array.
            ("(ly:grob-suicide! (g 4))", ""),
            (
                "(ly:pointer-group-interface::add-grob (g 4) 'probe-set (g 1))",
                "",
            ),
            ("(ly:grob-object (g 4) 'probe-set)", "()"),
        ],
    );
    // From Rust, as layout's engravers will add.
    let g: Vec<Value> = (0..4).map(|n| eval(&mut vm, &format!("(g {n})"))).collect();
    let [set, other, none] =
        ["probe-set", "probe-other", "probe-none"].map(|n| vm.heap_mut().intern(n));
    assert_eq!(count(&mut vm, g[1], set).unwrap(), 2);
    assert_eq!(count(&mut vm, g[1], none).unwrap(), 0);
    add_grob(&mut vm, g[1], set, g[2]).unwrap();
    assert_eq!(count(&mut vm, g[1], set).unwrap(), 3);
    let arr = get_grob_array(&mut vm, g[1], set).unwrap();
    assert!(vm.heap().foreign::<GrobArray>(arr).unwrap().ordered);
    add_unordered_grob(&mut vm, g[1], other, g[3]).unwrap();
    let arr = get_grob_array(&mut vm, g[1], other).unwrap();
    assert!(!vm.heap().foreign::<GrobArray>(arr).unwrap().ordered);
    set_ordered(&mut vm, g[1], other, true).unwrap();
    assert!(vm.heap().foreign::<GrobArray>(arr).unwrap().ordered);
    let found = find_grob(&mut vm, g[1], set, is_item).unwrap();
    assert_eq!(found, Some(g[1]));
    assert_eq!(
        extract_grob_array(&mut vm, g[1], "probe-set").unwrap(),
        [g[0], g[1], g[2]]
    );
    assert_eq!(extract_grob_array(&mut vm, g[1], "probe-none").unwrap(), []);
    // A grob of the wrong class is LilyPond's programming error, at the
    // grob: these, of no origin, as `ly:programming-error` gives them.
    eval(
        &mut vm,
        "(define (kept) (let ((d (car %diagnostics))) (set-car! %diagnostics '()) (map (lambda (v) (list (vector-ref v 0) (vector-ref v 1) (vector-ref v 2))) (reverse d))))
         (kept)",
    );
    set_class(vm.heap_mut(), g[2], Class::Spanner);
    assert_eq!(
        extract_item_array(&mut vm, g[1], "probe-set").unwrap(),
        [g[1]]
    );
    assert_eq!(
        extract_spanner_array(&mut vm, g[1], "probe-set").unwrap(),
        [g[2]]
    );
    let kept = eval(&mut vm, "(kept)");
    assert_eq!(
        show(&vm, kept),
        "((programming-error #f \"unexpected grob subtype in grob array\") \
         (programming-error #f \"unexpected grob subtype in grob array\") \
         (programming-error #f \"unexpected grob subtype in grob array\") \
         (programming-error #f \"unexpected grob subtype in grob array\"))"
    );
}

#[test]
fn the_units_are_lilyponds_in_millimetres() {
    let Some((mut vm, _layout)) = vm() else {
        return;
    };
    let mut caught = |call: &str| show_of(&mut vm, &caught(call));
    // As LilyPond 2.26.0 gives them.
    assert_eq!(
        caught("(map (lambda (f) (f 3)) (list ly:pt ly:mm ly:cm ly:inch ly:bp))"),
        "(1.054379410543794 2.9999999999999996 29.999999999999993 76.19999999999999 1.0583333333333331)"
    );
    assert_eq!(caught("(ly:bp 1/3)"), "0.11759259259259258");
    assert_eq!(caught("(ly:inch 2)"), "50.8");
    assert_eq!(
        caught("(ly:pt 1+2i)"),
        "(wrong-type-arg #f \"Wrong type (expecting ~A): ~S\" (\"real number\" 1.0+2.0i) (1.0+2.0i))"
    );
    assert_eq!(
        caught("(ly:pt 'x)"),
        "(wrong-type-arg \"ly:pt\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (1 \"number\" x) (x))"
    );
    assert_eq!(
        caught("(ly:cm \"1\")"),
        "(wrong-type-arg \"ly:cm\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (1 \"number\" \"1\") (\"1\"))"
    );
    assert_eq!(lily_engine_layout::INCH, 25.4);
    assert_eq!(lily_engine_layout::POINT * 72.27, 25.4);
}

/// What `text` gives, written.
fn show_of(vm: &mut Vm, text: &str) -> String {
    let v = eval(vm, text);
    show(vm, v)
}

// The pure calls. What LilyPond 2.26.0 gives for each is from
// $LILY_CORPUS/device-runs/2026-10-09-ms3-step14/pure/pure-probe.ly
// (pure-probe.stderr), the same calls on three TextSpanners (whose
// pure_y_extent is Grob's, as a grob of no class has here) and three
// TextScripts (items, which keep their pure height).

/// The calls that set the pure heights' properties on (g 0)–(g 2) and
/// read them, as the probe's block, and what LilyPond gives for each of a
/// grob whose pure height is `Grob::pure_y_extent`'s (`spanner`) or
/// `Item`'s (`item`).
const PURE_HEIGHTS: &[(&str, &str, &str)] = &[
    ("(ly:grob-set-parent! (g 1) 1 (g 0))", "", ""),
    ("(ly:grob-set-parent! (g 2) 1 (g 1))", "", ""),
    ("(ly:grob-set-property! (g 0) 'Y-offset 1)", "", ""),
    ("(ly:grob-set-property! (g 1) 'Y-offset 2)", "", ""),
    ("(ly:grob-set-property! (g 1) 'Y-extent '(0 . 1))", "", ""),
    (
        "(ly:grob-pure-height (g 1) (g 0) 0 1)",
        "(2.0 . 3.0)",
        "(2.0 . 3.0)",
    ),
    (
        "(ly:grob-pure-height (g 1) (g 1) 0 1 'v)",
        "(0.0 . 1.0)",
        "(0.0 . 1.0)",
    ),
    (
        "(ly:grob-set-property! (g 1) 'minimum-Y-extent '(-1 . 0))",
        "",
        "",
    ),
    (
        "(ly:grob-pure-height (g 1) (g 0) 0 1)",
        "(1.0 . 3.0)",
        "(2.0 . 3.0)",
    ),
    (
        "(ly:grob-set-property! (g 1) 'Y-extent (lambda (gr) '(0 . 4)))",
        "",
        "",
    ),
    (
        "(ly:grob-pure-height (g 1) (g 0) 0 1)",
        "(+inf.0 . -inf.0)",
        "(2.0 . 3.0)",
    ),
    (
        "(ly:grob-set-property! (g 1) 'Y-extent
           (ly:make-unpure-pure-container (lambda (gr) '(0 . 4)) (lambda (gr s e) (cons s e))))",
        "",
        "",
    ),
    (
        "(ly:grob-pure-height (g 1) (g 0) 3 5)",
        "(1.0 . 7.0)",
        "(2.0 . 3.0)",
    ),
    (
        "(ly:grob-set-property! (g 1) 'Y-offset
           (ly:make-unpure-pure-container (lambda (gr) 10) (lambda (gr s e) (+ s e))))",
        "",
        "",
    ),
    (
        "(ly:grob-pure-height (g 1) (g 0) 3 5)",
        "(7.0 . 13.0)",
        "(8.0 . 9.0)",
    ),
    (
        "(ly:grob-set-property! (g 1) 'Y-offset (lambda (gr) 10))",
        "",
        "",
    ),
    (
        "(ly:grob-pure-height (g 1) (g 0) 3 5)",
        "(-1.0 . 5.0)",
        "(0.0 . 1.0)",
    ),
    ("(ly:grob-set-property! (g 2) 'Y-extent '(0 . 1))", "", ""),
    ("(ly:grob-set-property! (g 2) 'Y-offset 1)", "", ""),
    (
        "(ly:grob-pure-height (g 2) (g 0) 0 1)",
        "(1.0 . 2.0)",
        "(1.0 . 2.0)",
    ),
    // An offset kept is the pure one too, and no alignment's.
    (
        "(ly:grob-relative-coordinate (g 1) (g 0) 1)",
        "10.0",
        "10.0",
    ),
    (
        "(ly:grob-pure-height (g 1) (g 0) 3 5)",
        "(9.0 . 15.0)",
        "(10.0 . 11.0)",
    ),
    (
        "(ly:grob-pure-height (g 2) (g 0) 0 1)",
        "(11.0 . 12.0)",
        "(11.0 . 12.0)",
    ),
    (
        "(ly:grob-property (g 2) 'pure-Y-offset-in-progress)",
        "()",
        "()",
    ),
    // A pure offset that reads itself: the cycle's programming error.
    (
        "(ly:grob-set-property! (g 2) 'Y-offset
           (ly:make-unpure-pure-container
            (lambda (gr) 0)
            (lambda (gr s e) (ly:grob-pure-height gr (ly:grob-parent gr 1) s e) 1)))",
        "",
        "",
    ),
    (
        "(ly:grob-pure-height (g 2) (g 0) 0 1)",
        "(11.0 . 12.0) (\"cyclic chain in pure-Y-offset callbacks\")",
        "(11.0 . 12.0) (\"cyclic chain in pure-Y-offset callbacks\")",
    ),
    (
        "(ly:grob-property (g 2) 'pure-Y-offset-in-progress)",
        "()",
        "()",
    ),
    // A dead grob: nothing of its own, but an item's height kept.
    ("(ly:grob-suicide! (g 2))", "", ""),
    (
        "(ly:grob-pure-property (g 2) 'Y-offset 0 1 'dead)",
        "dead",
        "dead",
    ),
    (
        "(ly:grob-pure-height (g 2) (g 2) 0 1)",
        "(+inf.0 . -inf.0)",
        "(0.0 . 1.0)",
    ),
];

/// [`glided`]'s properties with the pure heights' types.
fn pure_typed(vm: &mut Vm) {
    eval(
        vm,
        "(for-each (lambda (sym) (set-object-property! sym 'backend-type? pair?))
                   '(Y-extent minimum-Y-extent))",
    );
}

#[test]
fn the_pure_property_and_calls_give_what_lilypond_gives() {
    let Some((mut vm, _done)) = glided() else {
        return;
    };
    eval(
        &mut vm,
        "(define counted 0)
         (define upc (ly:make-unpure-pure-container (lambda (gr . rest) (list 'u rest))
                                                    (lambda (gr s e . rest) (list 'p s e rest))))
         (define unchanging (ly:make-unpure-pure-container (lambda (gr . rest) (list 'u rest))))",
    );
    let thrown = |call: &str| format!("(catch #t (lambda () {call}) (lambda (key . args) key))");
    check(
        &mut vm,
        &[
            ("(ly:grob-set-property! (g 0) 'padding 3)", ""),
            ("(ly:grob-pure-property (g 0) 'padding 0 1)", "3"),
            ("(ly:grob-pure-property (g 0) 'minimum-Y-extent 0 1)", "()"),
            (
                "(ly:grob-pure-property (g 0) 'minimum-Y-extent 0 1 'none)",
                "none",
            ),
            // A procedure is no pure value until the unpure lookup keeps
            // its value.
            ("(ly:grob-set-property! (g 0) 'padding (lambda (gr) 4))", ""),
            ("(ly:grob-pure-property (g 0) 'padding 0 1)", "#f"),
            ("(ly:grob-property (g 0) 'padding)", "4"),
            ("(ly:grob-pure-property (g 0) 'padding 0 1)", "4"),
            (
                "(ly:grob-set-property! (g 0) 'padding
                   (ly:make-unpure-pure-container (lambda (gr) 5) (lambda (gr s e) (list s e))))",
                "",
            ),
            ("(ly:grob-pure-property (g 0) 'padding 2 7)", "(2 7)"),
            ("(ly:grob-property (g 0) 'padding)", "5"),
            ("(ly:grob-pure-property (g 0) 'padding 2 7)", "5"),
            // A container without a pure part: the unpure lookup, kept.
            (
                "(ly:grob-set-property! (g 0) 'padding
                   (ly:make-unpure-pure-container (lambda (gr) (set! counted (1+ counted)) counted)))",
                "",
            ),
            ("(ly:grob-pure-property (g 0) 'padding 0 1)", "1"),
            ("(ly:grob-pure-property (g 0) 'padding 4 5)", "1"),
            (
                "(ly:grob-set-property! (g 0) 'padding (ly:make-unpure-pure-container 7))",
                "",
            ),
            ("(ly:grob-pure-property (g 0) 'padding 0 1)", "7"),
            (
                "(ly:grob-set-property! (g 0) 'padding
                   (ly:make-unpure-pure-container (lambda (gr) 8) (lambda (gr s e) '())))",
                "",
            ),
            ("(ly:grob-pure-property (g 0) 'padding 0 1 'none)", "none"),
            (
                "(ly:grob-set-property! (g 0) 'padding (ly:make-unpure-pure-container (lambda (gr) 8) 9))",
                "",
            ),
            ("(ly:grob-pure-property (g 0) 'padding 0 1)", "9"),
            // ly:pure-call and ly:unpure-call on a grob.
            ("(ly:pure-call upc (g 0) 1 2 'x)", "(p 1 2 (x))"),
            ("(ly:unpure-call upc (g 0) 'x)", "(u (x))"),
            (
                "(ly:pure-call (lambda (gr . rest) (list 'f rest)) (g 0) 1 2 'x)",
                "(f (x))",
            ),
            ("(ly:pure-call unchanging (g 0) 1 2 'x)", "(u (x))"),
            ("(ly:pure-call 3 (g 0) 1 2)", "3"),
            ("(ly:unpure-call 3 (g 0))", "3"),
            (
                "((ly:unpure-pure-container-pure-part unchanging) (g 0) 1 2 'x)",
                "(u (x))",
            ),
            (
                &thrown("(ly:pure-call (ly:unpure-pure-container-pure-part unchanging) (g 0) 1 2 'x)"),
                "wrong-number-of-args",
            ),
            // The arguments, checked and then converted.
            (
                &caught("(ly:grob-pure-property 'x 'padding 0 1)"),
                "(wrong-type-arg \"ly:grob-pure-property\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (1 \"Grob\" x) (x))",
            ),
            (
                &caught("(ly:grob-pure-property (g 0) \"padding\" 0 1)"),
                "(wrong-type-arg \"ly:grob-pure-property\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (2 \"symbol\" \"padding\") (\"padding\"))",
            ),
            (
                &caught("(ly:grob-pure-property (g 0) 'padding 0.5 1)"),
                "(wrong-type-arg \"ly:grob-pure-property\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (3 \"integer\" 0.5) (0.5))",
            ),
            (
                &caught("(ly:grob-pure-property (g 0) 'padding 1.0 1)"),
                "(wrong-type-arg #f \"Wrong type (expecting ~A): ~S\" (\"exact integer\" 1.0) (1.0))",
            ),
            (
                &caught("(ly:grob-pure-property (g 0) 'padding -1 1)"),
                "(out-of-range #f \"Value out of range ~S to< ~S: ~S\" (0 18446744073709551615 -1) (-1))",
            ),
            (
                &caught("(ly:grob-pure-property (g 0) 'padding 0 'x)"),
                "(wrong-type-arg \"ly:grob-pure-property\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (4 \"integer\" x) (x))",
            ),
            (
                &caught("(ly:grob-pure-property (g 0) 'padding 0 100000000000000000000)"),
                "(out-of-range #f \"Value out of range ~S to< ~S: ~S\" (0 18446744073709551615 100000000000000000000) (100000000000000000000))",
            ),
            (
                &caught("(ly:grob-pure-height (g 0) 'x 0 1)"),
                "(wrong-type-arg \"ly:grob-pure-height\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (2 \"Grob\" x) (x))",
            ),
            (
                &caught("(ly:grob-pure-height (g 0) (g 0) 0 -1)"),
                "(out-of-range #f \"Value out of range ~S to< ~S: ~S\" (0 18446744073709551615 -1) (-1))",
            ),
        ],
    );
}

#[test]
fn a_grobs_pure_height_is_lilyponds() {
    let Some((mut vm, _done)) = glided() else {
        return;
    };
    pure_typed(&mut vm);
    let calls: Vec<(&str, &str)> = PURE_HEIGHTS.iter().map(|&(c, s, _)| (c, s)).collect();
    check(&mut vm, &calls);
}

#[test]
fn an_item_keeps_its_pure_height_as_lilypond_does() {
    let Some((mut vm, _done)) = glided_with(CLASSED_DEFS, "(seq (note 0 2) (note 1 2))") else {
        return;
    };
    pure_typed(&mut vm);
    // (g 1) and (g 2) are NoteHeads, items; (g 0), a TextScript of two
    // classes, is the refpoint only.
    check(&mut vm, &[("(map ly:item? (list (g 1) (g 2)))", "(#t #t)")]);
    let calls: Vec<(&str, &str)> = PURE_HEIGHTS.iter().map(|&(c, _, i)| (c, i)).collect();
    check(&mut vm, &calls);
}

/// [`GLIDE_DEFS`] with a TextScript that is an alignment.
const ALIGNED_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (interfaces note-head-interface grob-interface)))
        (TextScript (meta (name . TextScript)
                          (interfaces align-interface grob-interface))))))",
    DEFS[1],
    GLIDE_DEFS[2],
];

#[test]
fn rust_reaches_the_pure_calls_through_a_host() {
    let Some((mut vm, _done)) = glided_with(ALIGNED_DEFS, "(seq (note 0 2))") else {
        return;
    };
    pure_typed(&mut vm);
    let g: Vec<Value> = (0..3).map(|n| eval(&mut vm, &format!("(g {n})"))).collect();
    let upc = eval(
        &mut vm,
        "(ly:make-unpure-pure-container (lambda (gr x) (list 'u x)) (lambda (gr s e x) (list 'p s e x)))",
    );
    let x = eval(&mut vm, "'x");
    let v = call_pure_function(&mut vm, upc, &[g[1], x], 2, 3).unwrap();
    assert_eq!(show(&vm, v), "(p 2 3 x)");
    let proc = eval(&mut vm, "(lambda (gr) 1)");
    let v = call_pure_function(&mut vm, proc, &[g[1]], 0, 1).unwrap();
    assert_eq!(v, Value::FALSE);
    eval(
        &mut vm,
        "(ly:grob-set-parent! (g 2) 1 (g 1))
         (ly:grob-set-property! (g 2) 'Y-offset (ly:make-unpure-pure-container (lambda (gr) 1) (lambda (gr s e) e)))
         (ly:grob-set-property! (g 2) 'Y-extent '(0 . 2))",
    );
    let sym = eval(&mut vm, "'Y-offset");
    let v = get_pure_property(&mut vm, g[2], sym, 0, 4).unwrap();
    assert_eq!(show(&vm, v), "4");
    assert_eq!(
        pure_relative_y_coordinate(&mut vm, g[2], g[1], 0, 4).unwrap(),
        4.0
    );
    assert_eq!(
        pure_y_extent(&mut vm, g[2], g[1], 0, 4).unwrap(),
        Interval {
            left: 4.0,
            right: 6.0
        }
    );
    // The child of an alignment is translated by it, by nothing for an
    // alignment without elements (MS3 step 28's, tests/layout.rs), and
    // its offset kept.
    eval(&mut vm, "(ly:grob-set-parent! (g 1) 1 (g 0))");
    assert_eq!(
        pure_relative_y_coordinate(&mut vm, g[1], g[0], 0, 1).unwrap(),
        0.0
    );
    eval(&mut vm, "(ly:grob-relative-coordinate (g 1) (g 0) 1)");
    assert_eq!(
        pure_relative_y_coordinate(&mut vm, g[1], g[0], 0, 1).unwrap(),
        0.0
    );
    assert_eq!(
        pure_relative_y_coordinate(&mut vm, g[2], g[0], 0, 4).unwrap(),
        4.0
    );
}

/// The calls of `grob-scheme.cc` that read and set a grob's alists, and
/// what LilyPond 2.26.0 gives for the same calls on a TextScript of no
/// cause ($LILY_CORPUS/device-runs/2026-10-09-ms3-step14/face/
/// face-probe.ly, .stderr): `ly:grob-property-data`,
/// `ly:grob-set-nested-property!`, `ly:grob-alist-chain`,
/// `ly:grob-basic-properties`, `ly:grob-layout` and `ly:grob-original`.
#[test]
fn a_grobs_alists_are_read_and_set_as_lilypond_does() {
    let Some((mut vm, done)) = glided() else {
        return;
    };
    check(
        &mut vm,
        &[
            ("(ly:grob-set-property! (g 2) 'padding (lambda (gr) 4))", ""),
            ("(procedure? (ly:grob-property-data (g 2) 'padding))", "#t"),
            ("(ly:grob-property-data (g 2) 'staff-padding)", "()"),
            ("(ly:grob-property-data (g 0) 'direction)", "1"),
            ("(ly:grob-property (g 2) 'padding)", "4"),
            ("(ly:grob-property-data (g 2) 'padding)", "4"),
            // set_nested_property: through the callbacks, unchecked; a
            // replaced sublist moves to the front.
            ("(ly:grob-set-nested-property! (g 2) '(details a b) 1)", ""),
            ("(ly:grob-property (g 2) 'details)", "((a (b . 1)))"),
            ("(ly:grob-set-nested-property! (g 2) '(details a c) 2)", ""),
            ("(ly:grob-property (g 2) 'details)", "((a (c . 2) (b . 1)))"),
            ("(ly:grob-set-nested-property! (g 2) '(details d) 3)", ""),
            (
                "(ly:grob-property (g 2) 'details)",
                "((d . 3) (a (c . 2) (b . 1)))",
            ),
            ("(ly:grob-set-nested-property! (g 2) '(padding) 5)", ""),
            ("(ly:grob-property (g 2) 'padding)", "5"),
            // One symbol is ly:grob-set-property!'s, checked.
            (
                "(ly:grob-set-nested-property! (g 2) '(padding) \"x\")",
                " (\"the property 'padding' must be of type 'number', ignoring invalid value '\\\"x\\\"'\")",
            ),
            ("(ly:grob-property (g 2) 'padding)", "5"),
            (
                &caught("(ly:grob-set-nested-property! (g 2) '(details \"a\") 1)"),
                "(wrong-type-arg \"ly_grob_set_nested_property_x\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (2 \"list of symbols\" (details \"a\")) ((details \"a\")))",
            ),
            (
                &caught("(ly:grob-set-nested-property! (g 2) '() 1)"),
                "(wrong-type-arg \"ly_grob_set_nested_property_x\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (2 \"list of symbols\" ()) (()))",
            ),
            (
                &caught("(ly:grob-property-data (g 2) \"padding\")"),
                "(wrong-type-arg \"ly:grob-property-data\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (2 \"symbol\" \"padding\") (\"padding\"))",
            ),
            // The chain: what was set, the constructor's skylines and
            // extents first (MS3 step 26), then the definition and the
            // global default. The cause of an event's
            // grob ends a copy of the mutable alist; one of no cause has
            // none, and its alist is the grob's own.
            (
                "(map car (car (ly:grob-alist-chain (g 2))))",
                "(details padding horizontal-skylines vertical-skylines Y-extent X-extent)",
            ),
            (
                "(let ((chain (ly:grob-alist-chain (g 2)))) (eq? (car chain) (car (ly:grob-alist-chain (g 2)))))",
                "#t",
            ),
            (
                "(map car (car (ly:grob-alist-chain (g 0))))",
                "(horizontal-skylines vertical-skylines Y-extent X-extent cause)",
            ),
            (
                "(eq? (cdr (assq 'cause (car (ly:grob-alist-chain (g 0))))) (ly:grob-property (g 0) 'cause))",
                "#t",
            ),
            (
                "(eq? (cdr (assq 'cause (car (ly:grob-alist-chain (g 1))))) (g 0))",
                "#t",
            ),
            ("(length (ly:grob-alist-chain (g 0)))", "3"),
            (
                "(caddr (ly:grob-alist-chain (g 0) '((a . 1))))",
                "((a . 1))",
            ),
            (
                "(eq? (cadr (ly:grob-alist-chain (g 0))) (ly:grob-basic-properties (g 0)))",
                "#t",
            ),
            (
                "(assq 'direction (ly:grob-basic-properties (g 0)))",
                "(direction . 1)",
            ),
            ("(eq? (ly:grob-original (g 0)) (g 0))", "#t"),
            ("(ly:output-def? (ly:grob-layout (g 0)))", "#t"),
            // A dead grob keeps its cause, and nothing else.
            ("(ly:grob-suicide! (g 0))", ""),
            ("(ly:grob-basic-properties (g 0))", "()"),
            ("(map car (car (ly:grob-alist-chain (g 0))))", "(cause)"),
            ("(cdr (ly:grob-alist-chain (g 0)))", "(() ())"),
        ],
    );
    // The layout is the translation's output definition; a copy's
    // original is the grob it was copied from.
    let g: Vec<Value> = (0..6).map(|n| eval(&mut vm, &format!("(g {n})"))).collect();
    let layout = eval(&mut vm, "(ly:grob-layout (g 2))");
    assert_eq!(layout, done.translation.arena.borrow().output_def());
    {
        let id = grob_of(vm.heap(), g[5]).unwrap().1;
        let mut a = done.translation.arena.borrow_mut();
        grobs(&mut a).get_mut(id).unwrap().original = Some(g[2]);
    }
    check(&mut vm, &[("(eq? (ly:grob-original (g 5)) (g 2))", "#t")]);
    // From Rust: the record's alists, as they are, then `global`.
    let global = eval(&mut vm, "'((a . 1))");
    let v = alist_chain(vm.heap_mut(), g[2], global);
    let id = grob_of(vm.heap(), g[2]).unwrap().1;
    let (props, definition) = {
        let a = done.translation.arena.borrow();
        let r = &a.records.grobs[id as usize];
        (r.props, r.definition)
    };
    let heap = vm.heap();
    let items: Vec<Value> = (0..3)
        .scan(v, |p, _| {
            let item = heap.car(*p);
            *p = heap.cdr(*p).unwrap_or(Value::NULL);
            item
        })
        .collect();
    assert_eq!(items, [props, definition, global]);
}

/// `ly:grob-interfaces`, `ly:grob-system` and
/// `ly:grob-spanned-column-rank-interval` of grobs of each class, as
/// LilyPond 2.26.0 gives them for an item and a spanner just made
/// (face-probe.ly): the class's interfaces in front of the definition's,
/// no system, and the empty ranks `(VPOS . 0)` of an item without a
/// column and `(0 . 0)` of a spanner without bounds.
#[test]
fn a_grobs_interfaces_system_and_ranks_are_its_classs() {
    let Some((mut vm, _done)) = glided_with(CLASSED_DEFS, "(seq (note 0 2) (note 1 2))") else {
        return;
    };
    let g: Vec<Value> = (0..6).map(|n| eval(&mut vm, &format!("(g {n})"))).collect();
    set_class(vm.heap_mut(), g[0], Class::Spanner);
    set_class(vm.heap_mut(), g[3], Class::System);
    set_class(vm.heap_mut(), g[5], Class::PaperColumn);
    check(
        &mut vm,
        &[
            (
                "(ly:grob-interfaces (g 1))",
                "(item-interface grob-interface note-head-interface)",
            ),
            (
                "(ly:grob-interfaces (g 0))",
                "(spanner-interface grob-interface text-script-interface)",
            ),
            (
                "(ly:grob-interfaces (g 3))",
                "(system-interface spanner-interface grob-interface text-script-interface)",
            ),
            (
                "(ly:grob-interfaces (g 5))",
                "(paper-column-interface item-interface grob-interface note-head-interface)",
            ),
            ("(ly:grob-system (g 1))", "()"),
            ("(ly:grob-system (g 0))", "()"),
            ("(ly:grob-system (g 3))", "()"),
            ("(ly:grob-system (g 5))", "()"),
            (
                "(ly:grob-spanned-column-rank-interval (g 1))",
                "(18446744073709551615 . 0)",
            ),
            ("(ly:grob-spanned-column-rank-interval (g 0))", "(0 . 0)"),
            ("(ly:grob-spanned-column-rank-interval (g 3))", "(0 . 0)"),
            // A column's rank is VPOS until step 15's add_column gives it
            // one, and so is its items'.
            (
                "(ly:grob-spanned-column-rank-interval (g 5))",
                "(18446744073709551615 . 18446744073709551615)",
            ),
            ("(ly:grob-set-parent! (g 2) 0 (g 5))", ""),
            ("(ly:grob-set-parent! (g 4) 0 (g 2))", ""),
            (
                "(ly:grob-spanned-column-rank-interval (g 4))",
                "(18446744073709551615 . 18446744073709551615)",
            ),
            // An item's system is its parent's on X: a column has none yet,
            // nor has a system without bounds (MS3 step 15: a system's is
            // its bounds', as `Spanner::get_system` gives it).
            ("(ly:grob-system (g 4))", "()"),
            ("(ly:grob-set-parent! (g 1) 0 (g 3))", ""),
            ("(ly:grob-system (g 1))", "()"),
            // A dead grob has no interfaces.
            ("(ly:grob-suicide! (g 1))", ""),
            ("(ly:grob-interfaces (g 1))", "()"),
            (
                &caught("(ly:grob-interfaces 'x)"),
                "(wrong-type-arg \"ly:grob-interfaces\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (1 \"Grob\" x) (x))",
            ),
            (
                &caught("(ly:grob-system 'x)"),
                "(wrong-type-arg \"ly:grob-system\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (1 \"Grob\" x) (x))",
            ),
        ],
    );
    let heap = vm.heap_mut();
    assert_eq!(item::get_system(heap, g[2]), None);
    assert_eq!(item::get_system(heap, g[3]), None);
    assert_eq!(
        item::spanned_column_rank_interval(heap, g[2]),
        (item::VPOS, item::VPOS)
    );
    let v = interfaces(heap, g[0]);
    assert_eq!(
        show(&vm, v),
        "(spanner-interface grob-interface text-script-interface)"
    );
}

/// `ly:grob-robust-relative-extent`, `ly:grob-common-refpoint-of-array`
/// and `ly:grob-pure-relative-coordinate`, as LilyPond 2.26.0 gives them
/// for the same calls (face-probe.ly), and from Rust.
#[test]
fn robust_extents_and_refpoints_are_lilyponds() {
    let Some((mut vm, _done)) = glided() else {
        return;
    };
    eval(
        &mut vm,
        "(set-object-property! 'X-extent 'backend-type? pair?)",
    );
    check(
        &mut vm,
        &[
            ("(ly:grob-set-property! (g 2) 'X-extent '(1 . 2))", ""),
            ("(ly:grob-set-property! (g 2) 'X-offset 3)", ""),
            (
                "(ly:grob-robust-relative-extent (g 2) (g 2) 0)",
                "(1.0 . 2.0)",
            ),
            (
                "(ly:grob-set-property! (g 5) 'X-extent '(+inf.0 . -inf.0))",
                "",
            ),
            ("(ly:grob-set-property! (g 5) 'X-offset 3)", ""),
            ("(ly:grob-set-parent! (g 5) 0 (g 2))", ""),
            (
                "(ly:grob-robust-relative-extent (g 5) (g 2) 0)",
                "(3.0 . 3.0)",
            ),
            (
                &caught("(ly:grob-robust-relative-extent (g 2) (g 5) 0)"),
                "(misc-error \"ly:grob-robust-relative-extent\" \"grob ~a is an invalid refpoint for ~a on ~a axis.\\nThe refpoint should be an ancestor (direct or indirect) of the grob.\" (\"NoteHead\" \"NoteHead\" \"X\") #f)",
            ),
            (
                &caught("(ly:grob-robust-relative-extent (g 2) (g 2) 2)"),
                "(wrong-type-arg \"ly:grob-robust-relative-extent\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (3 \"axis\" 2) (2))",
            ),
            (
                "(eq? (g 2) (ly:grob-common-refpoint-of-array (g 5) (ly:grob-list->grob-array (list (g 2) (g 5))) 0))",
                "#t",
            ),
            (
                "(eq? (g 5) (ly:grob-common-refpoint-of-array (g 5) (ly:grob-list->grob-array '()) 0))",
                "#t",
            ),
            (
                "(ly:grob-common-refpoint-of-array (g 5) (ly:grob-list->grob-array (list (g 1))) 0)",
                "#f",
            ),
            // Past a grob with no common refpoint, the next starts again.
            (
                "(eq? (g 2) (ly:grob-common-refpoint-of-array (g 5) (ly:grob-list->grob-array (list (g 1) (g 2))) 0))",
                "#t",
            ),
            (
                &caught("(ly:grob-common-refpoint-of-array (g 5) (list (g 2)) 0)"),
                "(wrong-type-arg \"ly:grob-common-refpoint-of-array\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (2 \"Grob_array\" (#<Grob NoteHead >)) ((#<Grob NoteHead >)))",
            ),
            ("(ly:grob-set-property! (g 5) 'Y-offset 2)", ""),
            ("(ly:grob-set-property! (g 2) 'Y-offset 1)", ""),
            ("(ly:grob-set-parent! (g 5) 1 (g 2))", ""),
            ("(ly:grob-pure-relative-coordinate (g 5) (g 2) 0 1)", "2.0"),
            (
                &caught("(ly:grob-pure-relative-coordinate (g 5) (g 2) -1 1)"),
                "(wrong-type-arg \"ly:grob-pure-relative-coordinate\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (3 \"integer in [0, 2^64)\" -1) (-1))",
            ),
            (
                &caught("(ly:grob-pure-relative-coordinate (g 5) (g 2) 0.0 1)"),
                "(wrong-type-arg \"ly:grob-pure-relative-coordinate\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (3 \"integer in [0, 2^64)\" 0.0) (0.0))",
            ),
            (
                &caught("(ly:grob-pure-relative-coordinate (g 5) (g 2) 0 'x)"),
                "(wrong-type-arg \"ly:grob-pure-relative-coordinate\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (4 \"integer in [0, 2^64)\" x) (x))",
            ),
        ],
    );
    let g: Vec<Value> = (0..6).map(|n| eval(&mut vm, &format!("(g {n})"))).collect();
    assert_eq!(
        robust_relative_extent(&mut vm, g[5], g[2], X).unwrap(),
        Interval {
            left: 3.0,
            right: 3.0
        }
    );
    // g1 shares no refpoint with g2, so g5 starts again.
    assert_eq!(
        common_refpoint_of_array(vm.heap(), &[g[1], g[5]], Some(g[2]), X),
        Some(g[5])
    );
}

/// [`GLIDE_DEFS`] with a TextScript that is an alignment and a NoteHead
/// that is an axis group.
const STACKED_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (interfaces axis-group-interface grob-interface)))
        (TextScript (meta (name . TextScript)
                          (interfaces align-interface grob-interface))))))",
    DEFS[1],
    GLIDE_DEFS[2],
];

/// `grob.cc`'s vertical ordering: `ly:grob-vertical<?` and
/// `ly:grob-get-vertical-axis-group-index` by the root alignment's
/// `elements`, or, in one axis group, by the coordinates; without an
/// alignment, LilyPond 2.26.0's programming error and `#f`, and -1
/// (face-probe.ly).
#[test]
fn grobs_are_ordered_vertically_by_their_axis_groups() {
    let Some((mut vm, _done)) = glided_with(STACKED_DEFS, "(seq (note 0 2) (note 1 2))") else {
        return;
    };
    check(
        &mut vm,
        &[
            (
                "(ly:grob-vertical<? (g 4) (g 5))",
                "#f (\"grob does not belong to a VerticalAlignment?\")",
            ),
            ("(ly:grob-get-vertical-axis-group-index (g 4))", "-1"),
            // (g 0) aligns the axis groups (g 1) and (g 2), whose elements
            // (g 4) and (g 5) are.
            ("(ly:grob-set-parent! (g 1) 1 (g 0))", ""),
            ("(ly:grob-set-parent! (g 2) 1 (g 0))", ""),
            ("(ly:grob-set-parent! (g 4) 1 (g 1))", ""),
            ("(ly:grob-set-parent! (g 5) 1 (g 2))", ""),
            ("(ly:grob-set-parent! (g 3) 1 (g 0))", ""),
            (
                "(ly:grob-set-object! (g 0) 'elements (ly:grob-list->grob-array (list (g 2) (g 1))))",
                "",
            ),
            ("(ly:grob-vertical<? (g 4) (g 5))", "#f"),
            ("(ly:grob-vertical<? (g 5) (g 4))", "#t"),
            ("(ly:grob-get-vertical-axis-group-index (g 4))", "1"),
            ("(ly:grob-get-vertical-axis-group-index (g 5))", "0"),
            // In one axis group, the higher coordinate is above.
            ("(ly:grob-set-property! (g 5) 'Y-offset 1)", ""),
            ("(ly:grob-set-property! (g 2) 'Y-offset 0)", ""),
            ("(ly:grob-vertical<? (g 5) (g 2))", "#t"),
            ("(ly:grob-vertical<? (g 2) (g 5))", "#f"),
            // (g 3), an alignment, is in no axis group: below (g 4)'s, or
            // not placed once (g 4)'s is not among the elements.
            ("(ly:grob-vertical<? (g 3) (g 4))", "#f"),
            (
                "(ly:grob-set-object! (g 0) 'elements (ly:grob-list->grob-array (list (g 2))))",
                "",
            ),
            (
                "(ly:grob-vertical<? (g 3) (g 4))",
                "#f (\"could not place this grob in its axis group\")",
            ),
            (
                "(ly:grob-get-vertical-axis-group-index (g 3))",
                "-1 (\"could not find this grob's vertical axis group in the vertical alignment\")",
            ),
            (
                &caught("(ly:grob-vertical<? (g 0) 'x)"),
                "(wrong-type-arg \"ly:grob-vertical<?\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (2 \"Grob\" x) (x))",
            ),
        ],
    );
    let g: Vec<Value> = (0..6).map(|n| eval(&mut vm, &format!("(g {n})"))).collect();
    assert!(vertical_less(&mut vm, g[5], g[4]).unwrap());
    assert!(!vertical_less(&mut vm, g[4], g[5]).unwrap());
    assert_eq!(get_vertical_axis_group_index(&mut vm, g[5]).unwrap(), 0);
}

/// `ly:grob-chain-callback`: `grob-closure.cc`'s `chain_callback` through
/// the installed `grob::compose-function`, with LilyPond 2.26.0's checks
/// (face-probe.ly).
#[test]
fn ly_grob_chain_callback_composes_through_the_image() {
    let Some((mut vm, _done)) = chained_over("(seq (note 0 2))") else {
        return;
    };
    check(
        &mut vm,
        &[
            (
                "(ly:grob-set-property! (g 0) 'staff-padding (lambda (gr) 2))",
                "",
            ),
            (
                "(ly:grob-chain-callback (g 0) (lambda (gr prev) (list 'chained prev)) 'staff-padding)",
                "",
            ),
            ("(ly:grob-property (g 0) 'staff-padding)", "(chained 2)"),
            (
                "(ly:grob-chain-callback (g 0) (lambda (gr prev) (list 'value prev)) 'direction)",
                "",
            ),
            ("(ly:grob-property (g 0) 'direction)", "(value 1)"),
            (
                "(ly:grob-chain-callback (g 0) (ly:make-unpure-pure-container (lambda (gr) 1)) 'padding)",
                "",
            ),
            ("(procedure? (ly:grob-property-data (g 0) 'padding))", "#f"),
            (
                &caught("(ly:grob-chain-callback (g 0) 3 'direction)"),
                "(wrong-type-arg \"ly_grob_chain_callback\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (2 \"procedure or unpure pure container\" 3) (3))",
            ),
            (
                &caught("(ly:grob-chain-callback (g 0) car \"direction\")"),
                "(wrong-type-arg \"ly:grob-chain-callback\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (3 \"symbol\" \"direction\") (\"direction\"))",
            ),
        ],
    );
}

/// `font.rs`' test pack, under the font name `name`.
fn font_pack(name: &str) -> Vec<u8> {
    const PACK: &[u8] = b"LILP\x03\x04test\x04Test\x00\xe8\x07\x03\x01\x0bdesign_size\x01\x90\x03\x02\x01a\x01b\xf4\x03\x00\xc3\x01\x00\x09\x01\xeb\x30\x98\x75\x00\xc1\x3e\x90\x4e\x00\x00\x06\x00\x00\x07\x01\x02\x02\x00\x00\x00";
    let mut pack = PACK[..5].to_vec();
    pack.push(name.len() as u8);
    pack.extend_from_slice(name.as_bytes());
    pack.extend_from_slice(&PACK[10..]);
    pack
}

/// `Font_interface::get_default_font` (`ly:grob-default-font`): the music
/// font of the grob's chain in its layout at staff size 20, kept as its
/// `font`, with the magnifications LilyPond 2.26.0 gives
/// (face-probe.ly); the packs are made by hand under Emmentaler's names.
#[test]
fn a_grobs_default_font_is_its_chains_music_font() {
    let Some((mut vm, _done)) = glided() else {
        return;
    };
    for name in ["emmentaler-20", "emmentaler-26"] {
        font::put_pack(&mut vm, &font_pack(name)).unwrap();
    }
    eval(
        &mut vm,
        "(define-public feta-design-size-mapping
           '((11 . 11.22) (13 . 12.60) (14 . 14.14) (16 . 15.87) (18 . 17.82) (20 . 20)
             (23 . 22.45) (26 . 25.20)))
         (let ((layout (ly:grob-layout (g 0))) (staff-height (* 20 (ly:pt 1))))
           (ly:output-def-set-variable! layout 'staff-height staff-height)
           (ly:output-def-set-variable! layout 'output-scale (/ staff-height 4)))
         (set-object-property! 'fonts 'backend-type? list?)
         (set-object-property! 'font-size 'backend-type? number?)
         (for-each (lambda (n) (ly:grob-set-property! (g n) 'fonts '((music . \"emmentaler\"))))
                   '(0 1))",
    );
    check(
        &mut vm,
        &[
            (
                "(let ((f (ly:grob-default-font (g 0)))) (list (ly:font-file-name f) (ly:font-magnification f)))",
                "(\"emmentaler-20\" 0.5690551181102361)",
            ),
            (
                "(eq? (ly:grob-default-font (g 0)) (ly:grob-property (g 0) 'font))",
                "#t",
            ),
            ("(ly:grob-set-property! (g 1) 'font-size 3)", ""),
            (
                "(let ((f (ly:grob-default-font (g 1)))) (list (ly:font-file-name f) (ly:font-magnification f)))",
                "(\"emmentaler-26\" 0.638702750610571)",
            ),
            // The chain ends with the music defaults.
            (
                "(caddr (%music-font-alist-chain (g 1)))",
                "((font-family . music) (font-encoding . fetaMusic))",
            ),
            (
                &caught("(ly:grob-default-font 'x)"),
                "(wrong-type-arg \"ly:grob-default-font\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (1 \"Grob\" x) (x))",
            ),
        ],
    );
    let g1 = eval(&mut vm, "(g 1)");
    let font = get_default_font(&mut vm, g1).unwrap();
    assert_eq!(font, eval(&mut vm, "(ly:grob-property (g 1) 'font)"));
    let chain = music_font_alist_chain(vm.heap_mut(), g1);
    assert_eq!(
        show(
            &vm,
            vm.heap()
                .car(vm.heap().cdr(vm.heap().cdr(chain).unwrap()).unwrap())
                .unwrap()
        ),
        "((font-family . music) (font-encoding . fetaMusic))"
    );
}

/// `ly:rest::width` (MS3 step 16, `rest.cc:281–289`), which
/// `Dot_column::add_head` takes of a dotted rest: the X extent of the
/// rest's glyph in its default font, named as `glyph_name` names it
/// without ledgers, and LilyPond's warning for a glyph the font has not,
/// whose extent is empty (as each of the test pack's is); an empty
/// stencil's extent for no duration log, with no warning; `scm_to_int`'s
/// error for a log that is no exact integer.
#[test]
fn a_rests_width_is_its_glyphs_as_lilypond_names_it() {
    let Some((mut vm, _done)) = glided() else {
        return;
    };
    font::put_pack(&mut vm, &font_pack("emmentaler-20")).unwrap();
    eval(
        &mut vm,
        "(define-public feta-design-size-mapping '((20 . 20)))
         (let ((layout (ly:grob-layout (g 0))) (staff-height (* 20 (ly:pt 1))))
           (ly:output-def-set-variable! layout 'staff-height staff-height)
           (ly:output-def-set-variable! layout 'output-scale (/ staff-height 4)))
         (set-object-property! 'fonts 'backend-type? list?)
         (for-each (lambda (sym) (set-object-property! sym 'backend-type? (lambda (x) #t)))
                   '(duration-log style font-size))
         (ly:grob-set-property! (g 0) 'fonts '((music . \"emmentaler\")))
         (define (width log style)
           (ly:grob-set-property! (g 0) 'duration-log log)
           (ly:grob-set-property! (g 0) 'style style)
           (ly:rest::width (g 0)))",
    );
    let not_found = |name: &str| format!("(+inf.0 . -inf.0) (\"rest `{name}' not found\")");
    check(
        &mut vm,
        &[
            ("(width 2 'default)", &not_found("rests.2")),
            ("(width 2 '())", &not_found("rests.2")),
            ("(width 2 \"x\")", &not_found("rests.2")),
            ("(width 2 'x)", &not_found("rests.2x")),
            // Classical and z differ from the default in quarter rests only.
            ("(width 2 'classical)", &not_found("rests.2classical")),
            ("(width 3 'classical)", &not_found("rests.3")),
            ("(width 2 'z)", &not_found("rests.2z")),
            ("(width 4 'z)", &not_found("rests.4")),
            // No mensural rest shorter than a 16th.
            ("(width 4 'mensural)", &not_found("rests.4mensural")),
            ("(width 5 'neomensural)", &not_found("rests.5")),
            // A breve's, moved by the staff space less its font size.
            ("(width -1 'default)", &not_found("rests.-1")),
            ("(width '() 'default)", "(+inf.0 . -inf.0)"),
            (
                &caught("(width 2.5 'default)"),
                "(wrong-type-arg #f \"Wrong type (expecting ~A): ~S\" (\"exact integer\" 2.5) (2.5))",
            ),
            (
                &caught("(ly:rest::width 1)"),
                "(wrong-type-arg \"ly:rest::width\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (1 \"Grob\" 1) (1))",
            ),
        ],
    );
}

/// A compile's grob (MS3 step 14a): translated as `Score::book_rendering`
/// translates, with the layout scaled by its book's paper's `output-scale`
/// and parented to that paper (`score.cc:135–142`), where `staff-height`
/// and `output-scale` are found; its default font is found through the
/// parent and kept in the root's `scaled-fonts`, as `find_scaled_font`
/// keeps it (`paper-def.cc:53–76`).
#[test]
fn a_compiled_grobs_default_font_is_found_through_its_layouts_parent() {
    let Some((mut vm, layout)) = vm_with(GLIDE_DEFS) else {
        return;
    };
    for name in ["emmentaler-20", "emmentaler-26"] {
        font::put_pack(&mut vm, &font_pack(name)).unwrap();
    }
    eval(
        &mut vm,
        "(define-public feta-design-size-mapping
           '((11 . 11.22) (13 . 12.60) (14 . 14.14) (16 . 15.87) (18 . 17.82) (20 . 20)
             (23 . 22.45) (26 . 25.20)))
         (define-public the-paper (ly:make-output-def))
         (let ((staff-height (* 20 (ly:pt 1))))
           (ly:output-def-set-variable! the-paper 'output-def-kind 'paper)
           (ly:output-def-set-variable! the-paper 'staff-height staff-height)
           (ly:output-def-set-variable! the-paper 'output-scale (/ staff-height 4)))
         (set-object-property! 'fonts 'backend-type? list?)
         (set-object-property! 'font-size 'backend-type? number?)",
    );
    vm.define("the-layout", layout);
    eval(
        &mut vm,
        "(ly:output-def-set-variable! the-layout 'output-def-kind 'layout)
         (ly:output-def-set-variable! the-layout 'mm 1)
         (ly:output-def-set-variable! the-layout 'dimension-variables '(mm))",
    );
    let paper = eval(&mut vm, "the-paper");
    let printed = paper_book(&mut vm, paper, None).unwrap();
    vm.heap_mut().root(printed);
    let scaled = scaled_layout(&mut vm, layout, printed).unwrap();
    vm.heap_mut().root(scaled);
    let (mut vm, _done) = glided_in(vm, scaled, "(seq (note 0 2) (note 1 2))");
    vm.define("printed", printed);
    eval(
        &mut vm,
        "(for-each (lambda (n) (ly:grob-set-property! (g n) 'fonts '((music . \"emmentaler\"))))
                   '(0 1))",
    );
    check(
        &mut vm,
        &[
            // The grob's layout is the scaled copy, its parent the paper.
            (
                "(eq? (ly:output-def-parent (ly:grob-layout (g 0))) printed)",
                "#t",
            ),
            (
                "(ly:output-def-lookup (ly:grob-layout (g 0)) 'mm)",
                "0.5690551181102361",
            ),
            (
                "(ly:output-def-lookup (ly:grob-layout (g 0)) 'cloned)",
                "#t",
            ),
            (
                "(let ((f (ly:grob-default-font (g 0)))) (list (ly:font-file-name f) (ly:font-magnification f)))",
                "(\"emmentaler-20\" 0.5690551181102361)",
            ),
            ("(ly:grob-set-property! (g 1) 'font-size 3)", ""),
            (
                "(let ((f (ly:grob-default-font (g 1)))) (list (ly:font-file-name f) (ly:font-magnification f)))",
                "(\"emmentaler-26\" 0.638702750610571)",
            ),
            // Kept in the root's scope, not the layout's.
            (
                "(not (ly:output-def-lookup printed 'scaled-fonts #f))",
                "#f",
            ),
            (
                "(module-ref (ly:output-def-scope (ly:grob-layout (g 0))) 'scaled-fonts #f)",
                "#f",
            ),
        ],
    );
    let root_scope = vm.heap().foreign::<OutputDef>(printed).unwrap().scope;
    let scaled_scope = vm.heap().foreign::<OutputDef>(scaled).unwrap().scope;
    assert!(vm.module_value(root_scope, "scaled-fonts").is_some());
    assert!(vm.module_value(scaled_scope, "scaled-fonts").is_none());
    // Without the parent, the layout alone has no `staff-height`: the
    // grob of a translation with it, after the six of the first.
    let (mut vm, _done) = glided_in(vm, layout, "(seq (note 0 2))");
    eval(
        &mut vm,
        "(ly:grob-set-property! (g 6) 'fonts '((music . \"emmentaler\")))",
    );
    check(
        &mut vm,
        &[(
            &caught("(ly:grob-default-font (g 6))"),
            "(wrong-type-arg #f \"Wrong type (expecting ~A): ~S\" (\"real number\" #<undefined>) (#<undefined>))",
        )],
    );
}

/// `(kept)`: the kind and text of each diagnostic given since the last
/// `(kept)`, which forgets them.
fn keep_diagnostics(vm: &mut Vm) {
    eval(
        vm,
        "(define (kept) (let ((d (car %diagnostics))) (set-car! %diagnostics '()) (map (lambda (v) (list (vector-ref v 0) (vector-ref v 2))) (reverse d))))
         (kept)",
    );
}

/// Part 3's functions that systems and paper columns are made of (MS3
/// step 15), as `system.cc`, `paper-score.cc`, `paper-column.cc`,
/// `axis-group-interface.cc` and `separation-item.cc` give them, on the
/// grobs of a translation that no engraver of layout's has typeset yet.
#[test]
fn systems_and_paper_columns_are_made_as_lilypond_makes_them() {
    use lily_engine_layout::paper_score::{self, PaperScore};
    use lily_engine_layout::{axis_group, paper_column, separation_item, system};
    let Some((mut vm, _done)) = glided_with(CLASSED_DEFS, "(seq (note 0 2) (note 1 2))") else {
        return;
    };
    keep_diagnostics(&mut vm);
    let g: Vec<Value> = (0..6).map(|n| eval(&mut vm, &format!("(g {n})"))).collect();
    let layout = grob_of(vm.heap(), g[0]).unwrap().0.borrow().output_def();
    // A system: `system-interface` after `spanner-interface`, and its
    // elements, an empty unordered array.
    system::make(vm.heap_mut(), g[0]);
    check(
        &mut vm,
        &[
            (
                "(ly:grob-interfaces (g 0))",
                "(system-interface spanner-interface grob-interface text-script-interface)",
            ),
            ("(ly:grob-object (g 0) 'all-elements)", "#<Grob_array>"),
        ],
    );
    let all = |vm: &mut Vm, s: Value| {
        let sym = vm.heap_mut().intern("all-elements");
        get_grob_array(vm, s, sym).unwrap()
    };
    let arr = all(&mut vm, g[0]);
    assert!(!vm.heap().foreign::<GrobArray>(arr).unwrap().ordered);
    // The Paper_score: its first system is its root, every system typeset
    // in it takes it and its layout.
    let ps = vm.heap_mut().wrap(PaperScore::new(layout));
    vm.heap_mut().root(ps);
    assert_eq!(show(&vm, ps), "#<Paper_score>");
    assert_eq!(paper_score::root_system(vm.heap(), ps), None);
    assert_eq!(paper_score::layout(vm.heap(), ps), Some(layout));
    paper_score::typeset_system(vm.heap_mut(), ps, g[0]);
    system::make(vm.heap_mut(), g[3]);
    paper_score::typeset_system(vm.heap_mut(), ps, g[3]);
    assert_eq!(paper_score::root_system(vm.heap(), ps), Some(g[0]));
    for s in [g[0], g[3]] {
        let (pscore, layout_of) = with_grob(vm.heap(), s, |g| (g.pscore, g.layout)).unwrap();
        assert_eq!((pscore, layout_of), (Some(ps), Some(layout)));
    }
    // A grob typeset into the system takes its layout, once.
    system::typeset_grob(&mut vm, g[0], g[1]).unwrap();
    assert_eq!(with_grob(vm.heap(), g[1], |g| g.layout), Some(Some(layout)));
    system::typeset_grob(&mut vm, g[0], g[1]).unwrap();
    let arr = all(&mut vm, g[0]);
    assert_eq!(vm.heap().foreign::<GrobArray>(arr).unwrap().grobs, [g[1]]);
    let seen = eval(&mut vm, "(kept)");
    assert_eq!(
        show(&vm, seen),
        "((programming-error \"adding element twice\"))"
    );
    // Columns: no system and the rank VPOS when made, then their rank in
    // the system's `columns`, the system their parent on its axes where
    // they have none.
    paper_column::make(vm.heap_mut(), g[2]);
    paper_column::make(vm.heap_mut(), g[5]);
    check(
        &mut vm,
        &[(
            "(ly:grob-interfaces (g 2))",
            "(paper-column-interface item-interface grob-interface note-head-interface)",
        )],
    );
    assert_eq!(paper_column::rank(vm.heap(), g[2]), item::VPOS);
    assert_eq!(paper_column::system(vm.heap(), g[2]), None);
    eval(&mut vm, "(%grob-set-property-data! (g 0) 'axes '(0 1))");
    set_parent(vm.heap(), g[5], g[1], Y);
    system::add_column(&mut vm, g[0], g[2]).unwrap();
    system::add_column(&mut vm, g[0], g[5]).unwrap();
    assert_eq!(paper_column::rank(vm.heap(), g[2]), 0);
    assert_eq!(paper_column::rank(vm.heap(), g[5]), 1);
    let heap = vm.heap();
    assert_eq!(get_parent(heap, g[2], X), Some(g[0]));
    assert_eq!(get_parent(heap, g[2], Y), Some(g[0]));
    assert_eq!(get_parent(heap, g[5], X), Some(g[0]));
    assert_eq!(get_parent(heap, g[5], Y), Some(g[1]));
    check(
        &mut vm,
        &[
            (
                "(ly:grob-object (g 0) 'columns)",
                "#<Grob_array#<Grob NoteHead > #<Grob NoteHead > >",
            ),
            (
                "(map (lambda (e) (eq? e (g 0))) (list (ly:grob-object (g 5) 'axis-group-parent-X) (ly:grob-object (g 5) 'axis-group-parent-Y)))",
                "(#t #t)",
            ),
            (
                "(equal? (ly:grob-array->list (ly:grob-object (g 0) 'elements)) (list (g 2) (g 5)))",
                "#t",
            ),
        ],
    );
    // No axes is LilyPond's programming error, the element still added;
    // an axis that is no integer, Guile's type error.
    axis_group::add_element(&mut vm, g[3], g[4]).unwrap();
    let seen = eval(&mut vm, "(kept)");
    assert_eq!(
        show(&vm, seen),
        "((programming-error \"axes should be nonempty\"))"
    );
    assert_eq!(get_parent(vm.heap(), g[4], X), None);
    assert_eq!(
        extract_grob_array(&mut vm, g[3], "elements").unwrap(),
        [g[4]]
    );
    eval(&mut vm, "(%grob-set-property-data! (g 3) 'axes '(x))");
    let e = axis_group::add_element(&mut vm, g[3], g[4]).unwrap_err();
    assert_eq!(e.text(vm.heap()), "Wrong type (expecting exact integer): x");
    // A copy of a column has its rank and no system; a copy of a system,
    // elements of its own.
    with_grob_mut(vm.heap(), g[4], |g| g.column.system = Some(Value::NULL));
    paper_column::copied(vm.heap(), g[4], g[5]);
    assert_eq!(paper_column::rank(vm.heap(), g[4]), 1);
    assert_eq!(paper_column::system(vm.heap(), g[4]), None);
    system::copied(vm.heap_mut(), g[3]);
    let arr = all(&mut vm, g[3]);
    assert!(
        vm.heap()
            .foreign::<GrobArray>(arr)
            .unwrap()
            .grobs
            .is_empty()
    );
    // A column is breakable where its `line-break-permission` is a symbol.
    assert!(!paper_column::is_breakable(&mut vm, g[2]).unwrap());
    eval(
        &mut vm,
        "(%grob-set-property-data! (g 2) 'line-break-permission 'allow)",
    );
    assert!(paper_column::is_breakable(&mut vm, g[2]).unwrap());
    eval(
        &mut vm,
        "(%grob-set-property-data! (g 2) 'line-break-permission #t)",
    );
    assert!(!paper_column::is_breakable(&mut vm, g[2]).unwrap());
    // Separation items' elements and conditional elements.
    separation_item::add_item(&mut vm, g[1], g[2]).unwrap();
    separation_item::add_conditional_item(&mut vm, g[1], g[5]).unwrap();
    assert_eq!(
        extract_grob_array(&mut vm, g[1], "elements").unwrap(),
        [g[2]]
    );
    assert_eq!(
        extract_grob_array(&mut vm, g[1], "conditional-elements").unwrap(),
        [g[5]]
    );
    // What layout keeps of a system and a column is traced with the
    // translation: a Paper_score and a column's system held by nothing
    // else, collected and allocated over.
    let other = vm.heap_mut().wrap(PaperScore::new(layout));
    let held = eval(&mut vm, "(list 'held)");
    with_grob_mut(vm.heap(), g[3], |g| g.pscore = Some(other));
    with_grob_mut(vm.heap(), g[4], |g| g.column.system = Some(held));
    vm.collect();
    eval(&mut vm, "(length (iota 100000))");
    assert_eq!(show(&vm, other), "#<Paper_score>");
    assert_eq!(show(&vm, held), "(held)");
}

/// Spanners (MS3 step 15): their bounds and the slots of their broken
/// pieces, as `spanner.cc` and `spanner-scheme.cc` give them, on the grobs
/// of a translation: (g 0), (g 3) and (g 6) spanners, (g 9) a system, (g
/// 2) and (g 5) columns, the other NoteHeads items. A system has no system
/// before breaking gives its columns one, and its bounds' columns' ranks,
/// as LilyPond 2.26.0 gives the root system's at the Score's finish
/// (`$LILY_CORPUS/device-runs/2026-10-09-ms3-step15/spanner/sys.ly`: `()`,
/// two NonMusicalPaperColumns, `(0 . 4)`, `()` and `#f`).
#[test]
fn a_spanner_has_bounds_and_the_slots_of_its_pieces() {
    use lily_engine_layout::spanner::{
        add_bound_item, broken_neighbor, find_broken_piece, get_bound, is_broken, set_bound,
    };
    use lily_engine_layout::{paper_column, system};
    let Some((mut vm, _done)) = glided_with(
        CLASSED_DEFS,
        "(seq (note 0 2) (note 1 2) (note 2 2) (note 3 2))",
    ) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let g: Vec<Value> = (0..12)
        .map(|n| eval(&mut vm, &format!("(g {n})")))
        .collect();
    for i in [0, 3, 6] {
        set_class(vm.heap_mut(), g[i], Class::Spanner);
    }
    system::make(vm.heap_mut(), g[9]);
    paper_column::make(vm.heap_mut(), g[2]);
    paper_column::make(vm.heap_mut(), g[5]);
    let wrong = |subr: &str, position: usize, what: &str, v: &str| {
        format!(
            "(wrong-type-arg \"{subr}\" \"Wrong type argument in position ~A (expecting ~A): ~S\" ({position} \"{what}\" {v}) ({v}))"
        )
    };
    check(
        &mut vm,
        &[
            (
                "(map ly:spanner? (list (g 0) (g 9) (g 1) (g 2) 1))",
                "(#t #t #f #f #f)",
            ),
            ("(ly:spanner-bound (g 0) -1)", "()"),
            ("(ly:spanner-bound (g 0) 1 'none)", "none"),
            ("(ly:spanner-broken-into (g 0))", "()"),
            ("(ly:spanner-broken-neighbor (g 0) 1)", "#f"),
            (
                &caught("(ly:spanner-bound (g 1) -1)"),
                &wrong("ly:spanner-bound", 1, "Spanner", "#<Grob NoteHead >"),
            ),
            (
                &caught("(ly:spanner-bound (g 0) 2)"),
                &wrong("ly:spanner-bound", 2, "direction", "2"),
            ),
            (
                &caught("(ly:spanner-set-bound! (g 0) -1 (g 3))"),
                &wrong("ly:spanner-set-bound!", 3, "Item", "#<Grob TextScript >"),
            ),
            (
                &caught("(ly:spanner-broken-into 'x)"),
                &wrong("ly:spanner-broken-into", 1, "Spanner", "x"),
            ),
            (
                &caught("(ly:spanner-broken-neighbor (g 0) 'x)"),
                &wrong("ly:spanner-broken-neighbor", 2, "direction", "x"),
            ),
            // The left bound is the spanner's parent on X, unless that is a
            // spanner; a column takes the spanner in its `bounded-by-me`.
            ("(ly:spanner-set-bound! (g 0) -1 (g 1))", ""),
            ("(eq? (ly:spanner-bound (g 0) -1) (g 1))", "#t"),
            ("(eq? (ly:grob-parent (g 0) 0) (g 1))", "#t"),
            ("(ly:spanner-set-bound! (g 0) 1 (g 2))", ""),
            ("(eq? (ly:spanner-bound (g 0) 1) (g 2))", "#t"),
            (
                "(equal? (ly:grob-array->list (ly:grob-object (g 2) 'bounded-by-me)) (list (g 0)))",
                "#t",
            ),
            ("(ly:grob-parent (g 0) 1)", "()"),
            ("(ly:grob-set-parent! (g 0) 0 (g 9))", ""),
            ("(ly:spanner-set-bound! (g 0) -1 (g 4))", ""),
            ("(eq? (ly:spanner-bound (g 0) -1) (g 4))", "#t"),
            ("(eq? (ly:grob-parent (g 0) 0) (g 9))", "#t"),
            // A system takes columns only.
            (
                "(ly:spanner-set-bound! (g 9) -1 (g 1))",
                " (\"cannot set NoteHead as bound of TextScript\")",
            ),
            ("(ly:spanner-bound (g 9) -1)", "()"),
        ],
    );
    // Through Rust: a system bounded by columns is no one's child by them;
    // a spanner is no bound; `add_bound_item` sets the left, then the right.
    set_bound(&mut vm, g[9], -1, g[2]).unwrap();
    set_bound(&mut vm, g[9], 1, g[5]).unwrap();
    assert_eq!(get_parent(vm.heap(), g[9], X), None);
    assert_eq!(get_bound(&mut vm, g[9], 1).unwrap(), Some(g[5]));
    set_bound(&mut vm, g[9], 1, g[3]).unwrap();
    assert_eq!(get_bound(&mut vm, g[9], 1).unwrap(), Some(g[5]));
    add_bound_item(&mut vm, g[6], g[7]).unwrap();
    add_bound_item(&mut vm, g[6], g[8]).unwrap();
    assert_eq!(get_bound(&mut vm, g[6], -1).unwrap(), Some(g[7]));
    assert_eq!(get_bound(&mut vm, g[6], 1).unwrap(), Some(g[8]));
    paper_column::set_rank(vm.heap(), g[2], 0);
    paper_column::set_rank(vm.heap(), g[5], 4);
    check(
        &mut vm,
        &[
            (
                "(kept)",
                "((programming-error \"cannot set TextScript as bound of TextScript\"))",
            ),
            (
                "(equal? (ly:grob-array->list (ly:grob-object (g 5) 'bounded-by-me)) (list (g 9)))",
                "#t",
            ),
            // A system's and its items' system is its bounds', none before
            // breaking; its ranks are its bounds' columns'.
            ("(ly:grob-system (g 9))", "()"),
            ("(ly:grob-spanned-column-rank-interval (g 9))", "(0 . 4)"),
            ("(ly:grob-spanned-column-rank-interval (g 0))", "(0 . 0)"),
            ("(ly:grob-set-parent! (g 10) 0 (g 9))", ""),
            ("(ly:grob-set-parent! (g 4) 0 (g 9))", ""),
            ("(ly:grob-system (g 10))", "()"),
        ],
    );
    // Once breaking gives the columns their system, it is the system's own,
    // its items' and a spanner's whose bounds are on it.
    for c in [2, 5] {
        with_grob_mut(vm.heap(), g[c], |col| col.column.system = Some(g[9]));
    }
    check(
        &mut vm,
        &[
            ("(eq? (ly:grob-system (g 9)) (g 9))", "#t"),
            ("(eq? (ly:grob-system (g 10)) (g 9))", "#t"),
            ("(eq? (ly:grob-system (g 0)) (g 9))", "#t"),
            ("(ly:grob-system (g 6))", "()"),
        ],
    );
    // The slots of the pieces, as breaking will fill them: (g 0) broken
    // into (g 3) and (g 6), (g 3) on (g 9). Collected and allocated over,
    // they are kept.
    with_grob_mut(vm.heap(), g[0], |s| s.broken_intos = vec![g[3], g[6]]);
    for (k, piece) in [3, 6].into_iter().enumerate() {
        with_grob_mut(vm.heap(), g[piece], |s| {
            (s.original, s.break_index) = (Some(g[0]), k);
        });
    }
    with_grob_mut(vm.heap(), g[3], |s| s.bounds = [Some(g[2]), Some(g[5])]);
    vm.collect();
    eval(&mut vm, "(length (iota 100000))");
    let heap = vm.heap_mut();
    assert!(is_broken(heap, g[0]));
    assert!(!is_broken(heap, g[3]));
    assert_eq!(find_broken_piece(heap, g[0], g[9]), Some(g[3]));
    assert_eq!(find_broken_piece(heap, g[3], g[9]), None);
    assert_eq!(broken_neighbor(heap, g[3], 1), Some(g[6]));
    assert_eq!(broken_neighbor(heap, g[6], -1), Some(g[3]));
    assert_eq!(broken_neighbor(heap, g[3], -1), None);
    assert_eq!(broken_neighbor(heap, g[0], 1), None);
    assert_eq!(item::find_broken_piece(heap, g[10], g[9]), Some(g[10]));
    assert_eq!(item::find_broken_piece(heap, g[1], g[9]), None);
    with_grob_mut(heap, g[1], |i| i.broken = [Some(g[10]), None]);
    assert_eq!(item::find_broken_piece(heap, g[1], g[9]), Some(g[10]));
    check(
        &mut vm,
        &[
            (
                "(equal? (ly:spanner-broken-into (g 0)) (list (g 3) (g 6)))",
                "#t",
            ),
            ("(eq? (ly:spanner-broken-neighbor (g 3) 1) (g 6))", "#t"),
            ("(ly:spanner-broken-neighbor (g 6) 1)", "#f"),
            ("(eq? (ly:grob-original (g 6)) (g 0))", "#t"),
        ],
    );
    // spanner.cc's callbacks, the music crate's placeholders once, under
    // LilyPond's names and arities. The normalized endpoints of the pieces
    // by their lengths: X-positions, else the bound infos' X, else the
    // bounds' coordinates; of what is not broken, (0 . 1).
    check(
        &mut vm,
        &[
            (
                "(list ly:spanner::calc-normalized-endpoints ly:spanner::kill-zero-spanned-time ly:spanner::set-spacing-rods)",
                "(#<procedure ly:spanner::calc-normalized-endpoints (a)> #<procedure ly:spanner::kill-zero-spanned-time (a)> #<procedure ly:spanner::set-spacing-rods (a)>)",
            ),
            ("(%grob-set-property-data! (g 3) 'X-positions '(0 . 2))", ""),
            (
                "(%grob-set-property-data! (g 6) 'left-bound-info '((X . 1.0)))",
                "",
            ),
            (
                "(%grob-set-property-data! (g 6) 'right-bound-info '((X . 4)))",
                "",
            ),
            (
                "(ly:spanner::calc-normalized-endpoints (g 3))",
                "(0.0 . 0.4)",
            ),
            (
                "(ly:grob-property (g 6) 'normalized-endpoints)",
                "(0.4 . 1.0)",
            ),
            (
                "(ly:spanner::calc-normalized-endpoints (g 6))",
                "(0.4 . 1.0)",
            ),
            (
                "(ly:spanner::calc-normalized-endpoints (g 9))",
                "(0.0 . 1.0)",
            ),
            (
                "(ly:grob-property (g 9) 'normalized-endpoints)",
                "(0.0 . 1.0)",
            ),
            // (g 6)'s bounds, (g 7) and (g 8), in the columns (g 2) and (g 5).
            ("(%grob-set-property-data! (g 6) 'left-bound-info '())", ""),
            ("(%grob-set-property-data! (g 6) 'right-bound-info '())", ""),
            ("(ly:grob-set-parent! (g 7) 0 (g 2))", ""),
            ("(ly:grob-set-parent! (g 8) 0 (g 5))", ""),
            ("(ly:grob-translate-axis! (g 8) -1 0)", ""),
            (
                "(ly:spanner::calc-normalized-endpoints (g 6))",
                "(1.0 . 1.0) (\"spanner with negative length\")",
            ),
            // `set-spacing-rods`: nothing without a minimum length; with
            // one, its rods (`tests/layout.rs`), none here, where (g 4)
            // has no column.
            ("(ly:spanner::set-spacing-rods (g 0))", ""),
            ("(%grob-set-property-data! (g 0) 'minimum-length 2)", ""),
            ("(ly:spanner::set-spacing-rods (g 0))", ""),
            ("(ly:grob-object (g 2) 'minimum-distances)", "()"),
            (
                &caught("(ly:spanner::set-spacing-rods (g 1))"),
                &wrong(
                    "ly:spanner::set-spacing-rods",
                    1,
                    "Spanner",
                    "#<Grob NoteHead >",
                ),
            ),
        ],
    );
    // `kill-zero-spanned-time`: a spanner whose left bound is a broken
    // copy, (g 7) of (g 1), dies when it spans no time.
    with_grob_mut(vm.heap(), g[7], |i| i.original = Some(g[1]));
    with_grob_mut(vm.heap(), g[1], |i| i.broken = [Some(g[7]), None]);
    check(
        &mut vm,
        &[
            ("(ly:item-break-dir (g 7))", "-1"),
            (
                "(%grob-set-property-data! (g 2) 'when (ly:make-moment 1/2))",
                "",
            ),
            (
                "(%grob-set-property-data! (g 5) 'when (ly:make-moment 3/4))",
                "",
            ),
            ("(ly:spanner::kill-zero-spanned-time (g 6))", ""),
            ("(%grob-live? (g 6))", "#t"),
            ("(ly:spanner::kill-zero-spanned-time (g 0))", ""),
            ("(%grob-live? (g 0))", "#t"),
            (
                "(%grob-set-property-data! (g 5) 'when (ly:make-moment 1/2))",
                "",
            ),
            ("(ly:spanner::kill-zero-spanned-time (g 6))", ""),
            ("(%grob-live? (g 6))", "#f"),
            // A dead spanner is named by its class.
            (
                "(catch #t (lambda () (ly:grob-extent (g 3) (g 6) 0)) (lambda (key subr message args rest) (list key subr args)))",
                "(misc-error \"ly:grob-extent\" (\"dead Spanner\" \"TextScript\" \"X\"))",
            ),
        ],
    );
}

/// [`CLASSED_DEFS`] with TextScript a sticky grob.
const STICKY_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface)))
        (TextScript (meta (name . TextScript) (classes Spanner)
                          (interfaces grob-interface sticky-grob-interface))))))",
    DEFS[1],
    GLIDE_DEFS[2],
];

/// A sticky spanner without a bound has its `sticky-host`'s, and LilyPond's
/// programming error if the host is no spanner.
#[test]
fn a_sticky_spanner_has_its_hosts_bounds() {
    use lily_engine_layout::spanner::get_bound;
    let Some((mut vm, _done)) = glided_with(STICKY_DEFS, "(seq (note 0 2) (note 1 2))") else {
        return;
    };
    keep_diagnostics(&mut vm);
    check(
        &mut vm,
        &[
            ("(ly:spanner? (g 0))", "#t"),
            (
                "(ly:spanner-bound (g 0) -1 'none)",
                "none (\"sticky spanner's host is not a spanner\")",
            ),
            ("(ly:spanner-set-bound! (g 3) -1 (g 1))", ""),
            ("(ly:spanner-set-bound! (g 3) 1 (g 4))", ""),
            ("(ly:grob-set-object! (g 0) 'sticky-host (g 3))", ""),
            ("(eq? (ly:spanner-bound (g 0) 1) (g 4))", "#t"),
            ("(kept)", "()"),
        ],
    );
    let (g0, g1) = (eval(&mut vm, "(g 0)"), eval(&mut vm, "(g 1)"));
    assert_eq!(get_bound(&mut vm, g0, -1).unwrap(), Some(g1));
    eval(&mut vm, "(ly:grob-set-object! (g 0) 'sticky-host (g 1))");
    assert_eq!(get_bound(&mut vm, g0, -1).unwrap(), None);
    check(
        &mut vm,
        &[(
            "(kept)",
            "((programming-error \"sticky spanner's host is not a spanner\"))",
        )],
    );
}

/// [`GLIDE_DEFS`] with a System's description, and `Parent_maker` in the
/// Voice too.
const SCORED_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface)))
        (TextScript (direction . 1)
                    (meta (name . TextScript) (classes Item)
                          (interfaces grob-interface text-script-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    DEFS[1],
    "((context-name Voice) (translator-type Engraver_group) (consists Glide_maker)
      (consists Parent_maker) (consists Score_engraver))",
];

/// `Score_engraver`'s layout part (MS3 step 15, `score-engraver.cc`): the
/// Score's `output`, a Paper_score of the translation's layout, and its
/// `rootSystem`, a System made from the Score's definition; each grob
/// announced typeset into it, in the order announced, and made an element
/// of it at the timestep's end unless it has a Y parent. A `\consists`
/// does not find it.
#[test]
fn score_engraver_typesets_the_grobs_into_its_system() {
    use lily_engine_layout::{grob_array, paper_score, score_engraver::SCORE_ENGRAVER};
    use lily_engine_music::GLOBAL;
    let Some((mut vm, layout)) = vm_with(SCORED_DEFS) else {
        return;
    };
    // `Parent_maker` makes, of each note, a TextScript and a NoteHead whose
    // Y parent is the TextScript, in `parented`.
    eval(
        &mut vm,
        "(define-public parented '())
         (ly:register-translator
           (lambda (context)
             (let ((heard #f))
               `((listeners (note-event . ,(lambda (t ev) (set! heard ev))))
                 (process-music
                  . ,(lambda (t)
                       (when heard
                         (let* ((text (ly:engraver-make-grob t 'TextScript heard))
                                (head (ly:engraver-make-grob t 'NoteHead text)))
                           (ly:grob-set-parent! head 1 text)
                           (set! parented (append parented (list text head))))
                         (set! heard #f)))))))
           'Parent_maker)",
    );
    keep_diagnostics(&mut vm);
    let music = eval(&mut vm, "(seq (note 0 2) (note 1 2))");
    vm.heap_mut().root(music);
    let done = run_translator_with(&mut vm, music, layout, None, &[&SCORE_ENGRAVER])
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    let seen = eval(&mut vm, "(kept)");
    assert_eq!(
        show(&vm, seen),
        "((warning \"unknown translator: `Score_engraver'\") (warning \"cannot find: `Score_engraver'\"))"
    );
    let (output, system) = {
        let a = done.translation.arena.borrow();
        let score = a.children(GLOBAL)[0];
        let heap = vm.heap_mut();
        let (output, root) = (heap.intern("output"), heap.intern("rootSystem"));
        (a.get(score, output), a.get(score, root))
    };
    assert_eq!(show(&vm, output), "#<Paper_score>");
    assert_eq!(show(&vm, system), "#<Grob System >");
    assert_eq!(paper_score::root_system(vm.heap(), output), Some(system));
    assert_eq!(paper_score::layout(vm.heap(), output), Some(layout));
    let made = |vm: &mut Vm, list: &str| -> Vec<Value> {
        let v = eval(vm, list);
        let mut out = Vec::new();
        let mut p = v;
        while let (Some(g), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
            out.push(g);
            p = rest;
        }
        out
    };
    let glided = made(&mut vm, "glided");
    let parented = made(&mut vm, "parented");
    assert_eq!((glided.len(), parented.len()), (6, 4));
    // Each timestep's grobs in the order announced: Glide_maker's three,
    // then Parent_maker's two.
    let mut announced = Vec::new();
    for n in 0..2 {
        announced.extend_from_slice(&glided[3 * n..3 * n + 3]);
        announced.extend_from_slice(&parented[2 * n..2 * n + 2]);
    }
    let elements =
        |vm: &mut Vm, name: &str| grob_array::extract_grob_array(vm, system, name).unwrap();
    assert_eq!(elements(&mut vm, "all-elements"), announced);
    for &g in &announced {
        assert_eq!(with_grob(vm.heap(), g, |g| g.layout), Some(Some(layout)));
    }
    // The System is the parent of each grob that had none; a NoteHead
    // whose Y parent is its TextScript is no element of it.
    let unparented: Vec<Value> = announced
        .iter()
        .copied()
        .filter(|g| !parented[1..].iter().step_by(2).any(|h| h == g))
        .collect();
    assert_eq!(unparented.len(), 8);
    assert_eq!(elements(&mut vm, "elements"), unparented);
    for &g in &unparented {
        assert_eq!(get_parent(vm.heap(), g, X), Some(system));
        assert_eq!(get_parent(vm.heap(), g, Y), Some(system));
    }
    for &h in parented[1..].iter().step_by(2) {
        assert_eq!(get_parent(vm.heap(), h, X), None);
    }
    assert_eq!(
        with_grob(vm.heap(), system, |g| g.pscore),
        Some(Some(output))
    );
}

/// [`SCORED_DEFS`] with the two columns' descriptions and a Score that
/// consists `Paper_column_engraver`, over a Voice of `Glide_maker`.
const COLUMN_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface)))
        (TextScript (direction . 1)
                    (meta (name . TextScript) (classes Item)
                          (interfaces grob-interface text-script-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice)
      (default-child Voice) (consists Paper_column_engraver))",
    "((context-name Voice) (translator-type Engraver_group) (consists Glide_maker))",
];

/// `Paper_column_engraver` (MS3 step 15, `paper-column-engraver.cc`): a
/// NonMusicalPaperColumn and a PaperColumn each timestep, the current
/// columns of the Score, added to the root system in that order with their
/// ranks, the first and the last command columns its bounds; `when` and
/// `measure-length`; each item it hears put on its timestep's musical
/// column, of which it is an element and whose rank it spans; a break
/// event's permission on the command column, `forceBreak` unset after the
/// timestep; and the last column's page permissions at the end. Layout's
/// class is found before the music crate's placeholder of its name.
#[test]
fn paper_column_engraver_makes_the_columns_and_puts_the_items_on_them() {
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::{grob_array, paper_column, spanner};
    use lily_engine_music::GLOBAL;
    let Some((mut vm, layout)) = vm_with(COLUMN_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let music = eval(
        &mut vm,
        "(seq (note 0 2)
              (make-music 'SimultaneousMusic 'elements
                (list (note 1 2) (make-music 'LineBreakEvent 'break-permission 'force))))",
    );
    vm.heap_mut().root(music);
    let classes = [&SCORE_ENGRAVER, &PAPER_COLUMN_ENGRAVER];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    let seen = eval(&mut vm, "(kept)");
    assert_eq!(show(&vm, seen), "()");
    let prop = |vm: &mut Vm, name: &str| {
        let a = done.translation.arena.borrow();
        let score = a.children(GLOBAL)[0];
        let sym = vm.heap_mut().intern(name);
        a.get(score, sym)
    };
    let system = prop(&mut vm, "rootSystem");
    let columns = grob_array::extract_grob_array(&mut vm, system, "columns").unwrap();
    let names: Vec<String> = columns.iter().map(|&c| show(&vm, c)).collect();
    assert_eq!(
        names,
        [
            "#<Grob NonMusicalPaperColumn >",
            "#<Grob PaperColumn >",
            "#<Grob NonMusicalPaperColumn >",
            "#<Grob PaperColumn >",
            "#<Grob NonMusicalPaperColumn >",
            "#<Grob PaperColumn >",
        ]
    );
    for (rank, &c) in columns.iter().enumerate() {
        assert_eq!(class(vm.heap_mut(), c), Some(Class::PaperColumn));
        assert_eq!(paper_column::rank(vm.heap(), c), rank as u64);
        assert_eq!(get_parent(vm.heap(), c, X), Some(system));
        assert_eq!(item::get_system(vm.heap_mut(), c), None);
    }
    // The music's end, at 1/2, is a timestep of its own.
    assert_eq!(prop(&mut vm, "currentCommandColumn"), columns[4]);
    assert_eq!(prop(&mut vm, "currentMusicalColumn"), columns[5]);
    assert_eq!(prop(&mut vm, "forceBreak"), Value::NULL);
    let bounds = [-1, 1].map(|d| spanner::get_bound(&mut vm, system, d).unwrap());
    assert_eq!(bounds, [Some(columns[0]), Some(columns[4])]);
    let get = |vm: &mut Vm, g: Value, name: &str| {
        let f = eval(vm, "ly:grob-property");
        let sym = vm.heap_mut().intern(name);
        let v = lily_engine_music::Host::call(vm, f, &[g, sym]).unwrap();
        show(vm, v)
    };
    for (c, when) in [
        (0, "#<Mom 0>"),
        (1, "#<Mom 0>"),
        (2, "#<Mom 1/4>"),
        (3, "#<Mom 1/4>"),
        (4, "#<Mom 1/2>"),
    ] {
        assert_eq!(get(&mut vm, columns[c], "when"), when);
    }
    // Without Timing_translator, each timestep is a measure's start.
    assert_eq!(get(&mut vm, columns[0], "measure-length"), "#<Mom 1>");
    assert_eq!(get(&mut vm, columns[0], "line-break-permission"), "allow");
    assert_eq!(get(&mut vm, columns[2], "line-break-permission"), "force");
    assert_eq!(get(&mut vm, columns[2], "page-turn-permission"), "()");
    assert_eq!(get(&mut vm, columns[4], "page-turn-permission"), "allow");
    // Each timestep's grobs on its musical column, its elements.
    let glided: Vec<Value> = {
        let v = eval(&mut vm, "glided");
        let mut out = Vec::new();
        let mut p = v;
        while let (Some(g), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
            out.push(g);
            p = rest;
        }
        out
    };
    assert_eq!(glided.len(), 6);
    for (n, &g) in glided.iter().enumerate() {
        let column = columns[1 + 2 * (n / 3)];
        assert_eq!(get_parent(vm.heap(), g, X), Some(column));
        assert_eq!(get_column(vm.heap_mut(), g), Some(column));
        let rank = (1 + 2 * (n / 3)) as u64;
        assert_eq!(
            item::spanned_column_rank_interval(vm.heap_mut(), g),
            (rank, rank)
        );
        assert!(!item::is_non_musical(&mut vm, g).unwrap());
    }
    for n in 0..2 {
        let elements = grob_array::extract_grob_array(&mut vm, columns[1 + 2 * n], "elements");
        assert_eq!(elements.unwrap(), &glided[3 * n..3 * n + 3]);
    }
    assert!(item::is_non_musical(&mut vm, columns[0]).unwrap());
}

/// Global with the grobs `Rhythm_maker` makes, of one class each, a Score
/// and a Voice with `Rhythm_maker`.
const RHYTHM_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface rhythmic-head-interface)))
        (Rest (X-extent . (0 . 1.5))
              (meta (name . Rest) (classes Item)
                    (interfaces grob-interface rest-interface rhythmic-head-interface)))
        (Stem (duration-log . 2)
              (meta (name . Stem) (classes Item) (interfaces grob-interface stem-interface)))
        (Dots (meta (name . Dots) (classes Item) (interfaces grob-interface dots-interface)))
        (NoteColumn (axes 0 1)
                    (meta (name . NoteColumn) (classes Item)
                          (interfaces grob-interface axis-group-interface note-column-interface)))
        (DotColumn (axes 0) (padding . 0.5)
                   (meta (name . DotColumn) (classes Item)
                         (interfaces grob-interface axis-group-interface dot-column-interface))))))",
    DEFS[1],
    "((context-name Voice) (translator-type Engraver_group) (consists Rhythm_maker))",
];

/// Part 3's and part 4's functions that note heads, stems, rests, dots and
/// note columns are joined with (MS3 step 16), as `stem.cc`,
/// `note-column.cc`, `dot-column.cc`, `rhythmic-head.cc` and
/// `side-position-interface.cc` give them, on the grobs `Rhythm_maker`
/// makes of two notes, which no engraver of layout's has joined yet: of
/// the note n, `(r 7n)` a NoteHead, `(r 7n+1)` a Rest, `(r 7n+2)` a Stem,
/// `(r 7n+3)` and `(r 7n+4)` Dots, `(r 7n+5)` a NoteColumn and `(r 7n+6)`
/// a DotColumn.
#[test]
fn stems_note_columns_and_dot_columns_take_their_heads_as_lilypond_does() {
    use lily_engine_layout::{dot_column, note_column, rhythmic_head, stem};
    let Some((mut vm, _done)) = glided_with(RHYTHM_DEFS, "(seq (note 0 2) (note 1 2))") else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, "(define (r n) (list-ref rhythm n))");
    let r: Vec<Value> = (0..14)
        .map(|n| eval(&mut vm, &format!("(r {n})")))
        .collect();
    let [head, rest, st, dots, rest_dots, col, dcol] = r[..7] else {
        unreachable!()
    };
    let [head2, rest2, st2, _, _, col2, dcol2] = r[7..] else {
        unreachable!()
    };
    // Rhythmic heads: no stem until a stem takes them; a `stem` that is no
    // item is none; the dots set as `dot`.
    assert_eq!(rhythmic_head::get_stem(&mut vm, head).unwrap(), None);
    stem::add_head(&mut vm, st, head).unwrap();
    stem::add_head(&mut vm, st, rest).unwrap();
    stem::add_head(&mut vm, st, dots).unwrap();
    assert_eq!(rhythmic_head::get_stem(&mut vm, head).unwrap(), Some(st));
    assert_eq!(rhythmic_head::get_stem(&mut vm, rest).unwrap(), Some(st));
    eval(&mut vm, "(ly:grob-set-object! (r 7) 'stem '(stem))");
    assert_eq!(rhythmic_head::get_stem(&mut vm, head2).unwrap(), None);
    rhythmic_head::set_dots(&mut vm, head, dots);
    check(
        &mut vm,
        &[
            ("(eq? (ly:grob-object (r 0) 'dot) (r 3))", "#t"),
            ("(ly:grob-object (r 1) 'dot)", "()"),
        ],
    );
    // A stem's note heads and rests; any grob added takes it as `stem`.
    assert_eq!(
        extract_grob_array(&mut vm, st, "note-heads").unwrap(),
        [head]
    );
    assert_eq!(extract_grob_array(&mut vm, st, "rests").unwrap(), [rest]);
    check(
        &mut vm,
        &[("(eq? (ly:grob-object (r 3) 'stem) (r 2))", "#t")],
    );
    assert_eq!(stem::head_count(&mut vm, st).unwrap(), 1);
    assert_eq!(stem::head_count(&mut vm, st2).unwrap(), 0);
    // A normal stem has heads and a duration log of 1 or more; the log is
    // 2 where it is no number, and Guile's errors where it is no `int`.
    assert!(stem::is_normal_stem(&mut vm, st).unwrap());
    assert!(!stem::is_normal_stem(&mut vm, st2).unwrap());
    assert_eq!(stem::duration_log(&mut vm, st).unwrap(), 2);
    let set = |vm: &mut Vm, n: usize, prop: &str, v: &str| {
        eval(
            vm,
            &format!("(%grob-set-property-data! (r {n}) '{prop} {v})"),
        );
    };
    set(&mut vm, 2, "duration-log", "0");
    assert!(!stem::is_normal_stem(&mut vm, st).unwrap());
    assert_eq!(stem::duration_log(&mut vm, st).unwrap(), 0);
    set(&mut vm, 2, "duration-log", "(lambda (grob) 1)");
    assert!(stem::is_normal_stem(&mut vm, st).unwrap());
    for v in ["'()", "'x"] {
        set(&mut vm, 9, "duration-log", v);
        assert_eq!(stem::duration_log(&mut vm, st2).unwrap(), 2, "{v}");
    }
    for (v, error) in [
        ("2.0", "Wrong type (expecting exact integer): 2.0"),
        ("5/2", "Wrong type (expecting exact integer): 5/2"),
        (
            "(expt 2 40)",
            "Value out of range -2147483648 to< 2147483647: 1099511627776",
        ),
    ] {
        set(&mut vm, 9, "duration-log", v);
        let e = stem::duration_log(&mut vm, st2).unwrap_err();
        assert_eq!(e.text(vm.heap()), error, "{v}");
    }
    set(&mut vm, 2, "duration-log", "'x");
    let e = stem::is_normal_stem(&mut vm, st).unwrap_err();
    assert_eq!(e.text(vm.heap()), "Wrong type (expecting exact integer): x");
    // Beaming: a pair made where there is none, each side's beams counted
    // down before what is there, `#f` for none.
    for (count, d, want) in [
        (2, 1, "(() 1 0)"),
        (1, -1, "((0) 1 0)"),
        (1, 1, "((0) 0 1 0)"),
        (0, -1, "(#f 0 1 0)"),
    ] {
        stem::set_beaming(&mut vm, st, count, d).unwrap();
        check(&mut vm, &[("(ly:grob-property (r 2) 'beaming)", want)]);
    }
    set(&mut vm, 9, "beaming", "(lambda (grob) (cons '(5) #f))");
    stem::set_beaming(&mut vm, st2, 1, -1).unwrap();
    check(
        &mut vm,
        &[("(ly:grob-property (r 9) 'beaming)", "((0 5) . #f)")],
    );
    // A note column: its stem and heads its elements on both axes, its
    // rest unless it has heads, and LilyPond's warning where a rest and
    // heads meet, the second not made an element.
    note_column::set_stem(&mut vm, col, st).unwrap();
    note_column::add_head(&mut vm, col, head).unwrap();
    note_column::add_head(&mut vm, col, rest).unwrap();
    note_column::add_head(&mut vm, col, dots).unwrap();
    assert_eq!(
        extract_grob_array(&mut vm, col, "elements").unwrap(),
        [st, head, dots]
    );
    assert_eq!(
        extract_grob_array(&mut vm, col, "note-heads").unwrap(),
        [head]
    );
    // The grobs' events have no origin: the warnings are unlocated.
    let warned = |vm: &mut Vm| {
        let kept = eval(vm, "(kept)");
        let seen = eval(vm, "(let ((s seen)) (set! seen '()) s)");
        format!("{} {}", show(vm, kept), show(vm, seen))
    };
    let both = "() (\"cannot have note heads and rests together on a stem\")";
    assert_eq!(warned(&mut vm), both);
    let heap = vm.heap();
    assert_eq!(get_parent(heap, st, X), Some(col));
    assert_eq!(get_parent(heap, st, Y), Some(col));
    assert_eq!(get_parent(heap, rest, X), None);
    check(
        &mut vm,
        &[
            ("(eq? (ly:grob-object (r 5) 'stem) (r 2))", "#t"),
            ("(ly:grob-object (r 5) 'rest)", "()"),
        ],
    );
    note_column::add_head(&mut vm, col2, rest2).unwrap();
    note_column::add_head(&mut vm, col2, head2).unwrap();
    assert_eq!(warned(&mut vm), both);
    assert_eq!(
        extract_grob_array(&mut vm, col2, "elements").unwrap(),
        [rest2]
    );
    assert_eq!(
        extract_grob_array(&mut vm, col2, "note-heads").unwrap(),
        [head2]
    );
    check(
        &mut vm,
        &[("(eq? (ly:grob-object (r 12) 'rest) (r 8))", "#t")],
    );
    // A dot column: a head without dots adds nothing; a note head's dots
    // positioned by their X parent on both axes, a rest's moved right by
    // its width and the column's padding; the heads its unordered support.
    dot_column::add_head(&mut vm, dcol, rest).unwrap();
    check(
        &mut vm,
        &[("(ly:grob-object (r 6) 'side-support-elements)", "()")],
    );
    dot_column::add_head(&mut vm, dcol, head).unwrap();
    rhythmic_head::set_dots(&mut vm, rest, rest_dots);
    dot_column::add_head(&mut vm, dcol, rest).unwrap();
    assert_eq!(
        extract_grob_array(&mut vm, dcol, "side-support-elements").unwrap(),
        [head, rest]
    );
    let sym = vm.heap_mut().intern("side-support-elements");
    let arr = get_grob_array(&mut vm, dcol, sym).unwrap();
    assert!(!vm.heap().foreign::<GrobArray>(arr).unwrap().ordered);
    assert_eq!(
        extract_grob_array(&mut vm, dcol, "dots").unwrap(),
        [dots, rest_dots]
    );
    assert_eq!(
        extract_grob_array(&mut vm, dcol, "elements").unwrap(),
        [dots, rest_dots]
    );
    check(
        &mut vm,
        &[
            (
                "(map (lambda (n) (eq? (ly:grob-property-data (r n) 'Y-offset) ly:grob::x-parent-positioning)) '(3 4))",
                "(#t #t)",
            ),
            (
                "(eq? (ly:grob-property-data (r 3) 'X-offset) ly:grob::x-parent-positioning)",
                "#t",
            ),
            ("(ly:grob-property-data (r 4) 'X-offset)", "()"),
            ("(eq? (ly:grob-parent (r 4) 0) (r 6))", "#t"),
            ("(ly:grob-relative-coordinate (r 4) (r 6) 0)", "2.0"),
        ],
    );
    // The padding a number, else Guile's error; an infinite move,
    // LilyPond's programming error at the dots, which stay.
    eval(&mut vm, "(ly:grob-set-object! (r 8) 'dot (r 10))");
    set(&mut vm, 13, "padding", "'x");
    let e = dot_column::add_head(&mut vm, dcol2, rest2).unwrap_err();
    assert_eq!(e.text(vm.heap()), "Wrong type (expecting real number): x");
    set(&mut vm, 13, "padding", "+inf.0");
    dot_column::add_head(&mut vm, dcol2, rest2).unwrap();
    assert_eq!(
        warned(&mut vm),
        "((programming-error \"Infinity or NaN encountered\")) ()"
    );
    check(
        &mut vm,
        &[("(ly:grob-relative-coordinate (r 10) (r 13) 0)", "0.0")],
    );
}

/// Global with NoteHead's, ApproximatePitchNoteHead's, Rest's and the
/// System's descriptions, and a Voice that consists `Note_heads_engraver`
/// and `Rest_engraver` with `middleCPosition` -6.
const HEADS_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface rhythmic-head-interface)))
        (ApproximatePitchNoteHead
         (meta (name . ApproximatePitchNoteHead) (classes Item)
               (interfaces grob-interface note-head-interface rhythmic-head-interface)))
        (Rest (meta (name . Rest) (classes Item)
                    (interfaces grob-interface rest-interface rhythmic-head-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice)
      (default-child Voice))",
    "((context-name Voice) (translator-type Engraver_group) (consists Note_heads_engraver)
      (consists Rest_engraver) (assign middleCPosition -6))",
];

/// `Note_heads_engraver` and `Rest_engraver` (MS3 step 16,
/// `note-heads-engraver.cc`, `rest-engraver.cc`), layout's classes found
/// before the music crate's placeholders: a NoteHead of each note, an
/// ApproximatePitchNoteHead of one whose pitch is approximate, caused by
/// the note, its `staff-position` from `staffLineLayoutFunction` (the
/// pitch's steps without one, 0 without a pitch) and `middleCPosition`,
/// its `style` from `shapeNoteStyles` by its step above `tonic`; a Rest of
/// the first rest of a timestep, the others warned of, at its pitch's
/// steps and `middleCPosition`, and none at the timestep after.
#[test]
fn note_heads_and_rests_are_engraved_as_lilypond_does() {
    use lily_engine_layout::note_heads_engraver::NOTE_HEADS_ENGRAVER;
    use lily_engine_layout::rest_engraver::REST_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_music::GLOBAL;
    let Some((mut vm, layout)) = vm_with(HEADS_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let music = eval(
        &mut vm,
        "(for-each (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
                   (list (cons 'middleCPosition number?)
                         (cons 'staffLineLayoutFunction procedure?)
                         (cons 'shapeNoteStyles vector?)
                         (cons 'tonic ly:pitch?)))
         (define (rest octave step log)
           (make-music 'RestEvent 'pitch (ly:make-pitch octave step 0)
                       'duration (ly:make-duration log)))
         (define (set name value) (make-music 'PropertySet 'symbol name 'value value))
         (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
         (make-music 'ContextSpeccedMusic 'context-type 'Voice 'element
          (seq (par (note 0 2) (note 2 2)
                   (make-music 'NoteEvent 'pitch (ly:make-pitch 1 0 0)
                               'duration (ly:make-duration 2) 'pitch-approximate #t)
                   (make-music 'NoteEvent 'duration (ly:make-duration 2))
                   (rest 1 0 2))
              (set 'staffLineLayoutFunction (lambda (p) (* 2 (ly:pitch-steps p))))
              (set 'shapeNoteStyles #(do re mi fa #f la ti))
              (set 'tonic (ly:make-pitch 0 1 0))
              (par (note 0 2) (note 2 2) (note 5 2)
                   (rest 0 0 2) (make-music 'RestEvent 'duration (ly:make-duration 2)))
              (note 1 2)
              (make-music 'RestEvent 'duration (ly:make-duration 2))))",
    );
    vm.heap_mut().root(music);
    let classes = [&SCORE_ENGRAVER, &NOTE_HEADS_ENGRAVER, &REST_ENGRAVER];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    let seen = eval(&mut vm, "(kept)");
    assert_eq!(
        show(&vm, seen),
        "((warning \"conflict with event: `rest-event'\") (warning \"discarding event: `rest-event'\"))"
    );
    let system = {
        let a = done.translation.arena.borrow();
        let score = a.children(GLOBAL)[0];
        let sym = vm.heap_mut().intern("rootSystem");
        a.get(score, sym)
    };
    let made = extract_grob_array(&mut vm, system, "all-elements").unwrap();
    let get = |vm: &mut Vm, g: Value, name: &str| {
        let f = eval(vm, "ly:grob-property");
        let sym = vm.heap_mut().intern(name);
        lily_engine_music::Host::call(vm, f, &[g, sym]).unwrap()
    };
    let got: Vec<String> = made
        .iter()
        .map(|&g| {
            let cause = get(&mut vm, g, "cause");
            let class = lily_engine_music::event_property(vm.heap_mut(), cause, "class");
            let [position, style] = ["staff-position", "style"].map(|p| get(&mut vm, g, p));
            format!(
                "{} {} {} {}",
                show(&vm, g),
                show(&vm, vm.heap().car(class).unwrap()),
                show(&vm, position),
                show(&vm, style),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            // The pitches' steps, middle C's at -6; no pitch, 0.
            "#<Grob NoteHead > note-event -6 ()",
            "#<Grob NoteHead > note-event -4 ()",
            "#<Grob ApproximatePitchNoteHead > note-event 1 ()",
            "#<Grob NoteHead > note-event -6 ()",
            "#<Grob Rest > rest-event 1 ()",
            // Twice the steps from the layout function; the styles by the
            // step above D, none for #f; the first rest only.
            "#<Grob NoteHead > note-event -6 ti",
            "#<Grob NoteHead > note-event -2 re",
            "#<Grob NoteHead > note-event 4 ()",
            "#<Grob Rest > rest-event -6 ()",
            "#<Grob NoteHead > note-event -4 do",
            // A rest without a pitch has no staff position.
            "#<Grob Rest > rest-event () ()",
        ]
    );
}

/// Global with the descriptions of the grobs `Stem_engraver` makes and
/// joins, and a Voice that consists `Note_heads_engraver`, `Rest_engraver`
/// and `Stem_engraver`.
const STEM_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface rhythmic-head-interface)))
        (Rest (meta (name . Rest) (classes Item)
                    (interfaces grob-interface rest-interface rhythmic-head-interface)))
        (Stem (meta (name . Stem) (classes Item) (interfaces grob-interface stem-interface)))
        (StemStub (meta (name . StemStub) (classes Item) (interfaces grob-interface)))
        (Flag (meta (name . Flag) (classes Item) (interfaces grob-interface flag-interface)))
        (StemTremolo (meta (name . StemTremolo) (classes Item)
                           (interfaces grob-interface stem-tremolo-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice)
      (default-child Voice))",
    "((context-name Voice) (translator-type Engraver_group) (consists Note_heads_engraver)
      (consists Rest_engraver) (consists Stem_engraver))",
];

/// `Stem_engraver` (MS3 step 16, `stem-engraver.cc`), a layout class found
/// before the music crate's placeholder: a Stem and a StemStub of each
/// timestep's first rhythmic head, caused by it, a rest's too; the other
/// heads added to it, with LilyPond's two warnings for one of another
/// duration's stem; a Flag of a normal stem of an eighth; a StemTremolo
/// of the tremolo heard, with its flags, or a warning for none; and the
/// beam counts that `\set` sets put in the stem's `beaming` and unset, so
/// that the next stem has none.
#[test]
fn stems_are_engraved_with_their_beam_counts_as_lilypond_does() {
    use lily_engine_layout::note_heads_engraver::NOTE_HEADS_ENGRAVER;
    use lily_engine_layout::rest_engraver::REST_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::stem_engraver::STEM_ENGRAVER;
    use lily_engine_music::GLOBAL;
    let Some((mut vm, layout)) = vm_with(STEM_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let music = eval(
        &mut vm,
        "(for-each (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
                   (list (cons 'stemLeftBeamCount integer?)
                         (cons 'stemRightBeamCount integer?)))
         (set-object-property! 'Stem 'is-grob? #t)
         (set-object-property! 'duration-log 'backend-type? integer?)
         (define (set name value) (make-music 'PropertySet 'symbol name 'value value))
         (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
         (define (tremolo type) (make-music 'TremoloEvent 'tremolo-type type))
         (make-music 'ContextSpeccedMusic 'context-type 'Voice 'element
          (seq (make-music 'OverrideProperty 'symbol 'Stem 'grob-property-path '(duration-log)
                           'grob-value
                           (lambda (g)
                             (ly:duration-log
                              (ly:event-property (ly:grob-property (ly:grob-property g 'cause) 'cause)
                                                 'duration))))
               (par (note 0 2) (note 2 2))
               (set 'stemLeftBeamCount 2)
               (set 'stemRightBeamCount 1)
               (note 0 3)
               (note 1 3)
               (par (note 0 2) (note 1 3))
               (par (note 0 0) (note 1 -1))
               (make-music 'RestEvent 'duration (ly:make-duration 2))
               (par (note 0 3) (tremolo 32))
               (par (note 0 2) (tremolo 4))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &NOTE_HEADS_ENGRAVER,
        &REST_ENGRAVER,
        &STEM_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    let seen = eval(&mut vm, "(kept)");
    assert_eq!(
        show(&vm, seen),
        "((warning \"adding note head to incompatible stem (type = 1/4)\") \
          (warning \"maybe input should specify polyphonic voices\") \
          (warning \"tremolo duration is too long\"))"
    );
    let system = {
        let a = done.translation.arena.borrow();
        let score = a.children(GLOBAL)[0];
        let sym = vm.heap_mut().intern("rootSystem");
        a.get(score, sym)
    };
    let made = extract_grob_array(&mut vm, system, "all-elements").unwrap();
    let show_of = |vm: &mut Vm, text: &str, g: Value| {
        let f = eval(vm, text);
        let v = lily_engine_music::Host::call(vm, f, &[g]).unwrap();
        show(vm, v)
    };
    let made: Vec<Value> = made
        .into_iter()
        .filter(|&g| !show(&vm, g).contains("NoteHead") && !show(&vm, g).contains("Rest"))
        .collect();
    let got: Vec<String> = made
        .iter()
        .map(|&g| {
            let parts = [
                "(lambda (g) (let ((c (ly:grob-property g 'cause))) (if (ly:grob? c) c (ly:event-property c 'class))))",
                "(lambda (g) (length (ly:grob-array->list (or (ly:grob-object g 'note-heads #f) (ly:grob-list->grob-array '())))))",
                "(lambda (g) (length (ly:grob-array->list (or (ly:grob-object g 'rests #f) (ly:grob-list->grob-array '())))))",
                "(lambda (g) (ly:grob-property g 'beaming))",
                "(lambda (g) (list (ly:grob? (ly:grob-object g 'flag)) (ly:grob? (ly:grob-object g 'tremolo-flag)) (ly:grob-property g 'flag-count)))",
            ]
            .map(|p| show_of(&mut vm, p, g));
            format!("{} {}", show(&vm, g), parts.join(" "))
        })
        .collect();
    // Each: the grob, its cause, its note heads and rests, its beaming,
    // whether it has a flag and a tremolo, and its flag count.
    assert_eq!(
        got,
        [
            // A chord's stem takes both heads.
            "#<Grob Stem > #<Grob NoteHead > 2 0 () (#f #f ())",
            "#<Grob StemStub > #<Grob NoteHead > 0 0 () (#f #f ())",
            // The beam counts set, then unset: the next stem has none.
            "#<Grob Stem > #<Grob NoteHead > 1 0 ((1 0) 0) (#t #f ())",
            "#<Grob StemStub > #<Grob NoteHead > 0 0 () (#f #f ())",
            "#<Grob Flag > #<Grob Stem > 0 0 () (#f #f ())",
            "#<Grob Stem > #<Grob NoteHead > 1 0 () (#t #f ())",
            "#<Grob StemStub > #<Grob NoteHead > 0 0 () (#f #f ())",
            "#<Grob Flag > #<Grob Stem > 0 0 () (#f #f ())",
            // The quarter's stem takes the eighth, warned of, with no flag.
            "#<Grob Stem > #<Grob NoteHead > 2 0 () (#f #f ())",
            "#<Grob StemStub > #<Grob NoteHead > 0 0 () (#f #f ())",
            // A whole note's and a breve's: compatible, no flag.
            "#<Grob Stem > #<Grob NoteHead > 2 0 () (#f #f ())",
            "#<Grob StemStub > #<Grob NoteHead > 0 0 () (#f #f ())",
            // A rest's, which takes it as a rest.
            "#<Grob Stem > #<Grob Rest > 0 1 () (#f #f ())",
            "#<Grob StemStub > #<Grob Rest > 0 0 () (#f #f ())",
            // A 32nd's tremolo on an eighth: two flags.
            "#<Grob Stem > #<Grob NoteHead > 1 0 () (#t #t ())",
            "#<Grob StemStub > #<Grob NoteHead > 0 0 () (#f #f ())",
            "#<Grob StemTremolo > (tremolo-event music-event StreamEvent) 0 0 () (#f #f 2)",
            "#<Grob Flag > #<Grob Stem > 0 0 () (#f #f ())",
            // A quarter's on a quarter: none, warned of.
            "#<Grob Stem > #<Grob NoteHead > 1 0 () (#f #f ())",
            "#<Grob StemStub > #<Grob NoteHead > 0 0 () (#f #f ())",
        ]
    );
}

/// Global with the descriptions of the grobs `Dots_engraver` and
/// `Dot_column_engraver` make and join, and a Voice that consists
/// `Note_heads_engraver`, `Rest_engraver`, `Dots_engraver` and
/// `Dot_column_engraver`.
const DOTS_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface rhythmic-head-interface)))
        (Rest (X-extent . (0 . 1.5))
              (meta (name . Rest) (classes Item)
                    (interfaces grob-interface rest-interface rhythmic-head-interface)))
        (Dots (meta (name . Dots) (classes Item) (interfaces grob-interface dots-interface)))
        (DotColumn (axes 0) (padding . 0.5)
                   (meta (name . DotColumn) (classes Item)
                         (interfaces grob-interface axis-group-interface dot-column-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice)
      (default-child Voice))",
    "((context-name Voice) (translator-type Engraver_group) (consists Note_heads_engraver)
      (consists Rest_engraver) (consists Dots_engraver) (consists Dot_column_engraver))",
];

/// `Dots_engraver` and `Dot_column_engraver` (MS3 step 16,
/// `dots-engraver.cc`, `dot-column-engraver.cc`), layout's classes found
/// before the music crate's placeholders: Dots of each rhythmic head whose
/// duration has dots, caused by it, its `dot` and put on it in Y; a
/// DotColumn of each timestep with dots, with no cause, taking each dotted
/// head, a rest's dots moved right by its width and the padding; none of a
/// timestep without dots.
#[test]
fn dots_and_dot_columns_are_engraved_as_lilypond_does() {
    use lily_engine_layout::dot_column_engraver::DOT_COLUMN_ENGRAVER;
    use lily_engine_layout::dots_engraver::DOTS_ENGRAVER;
    use lily_engine_layout::note_heads_engraver::NOTE_HEADS_ENGRAVER;
    use lily_engine_layout::rest_engraver::REST_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_music::GLOBAL;
    let Some((mut vm, layout)) = vm_with(DOTS_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let music = eval(
        &mut vm,
        "(define (dotted step log dots)
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 step 0)
                       'duration (ly:make-duration log dots)))
         (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
         (make-music 'ContextSpeccedMusic 'context-type 'Voice 'element
          (seq (par (dotted 0 2 1) (note 2 2) (dotted 4 2 2)
                    (make-music 'RestEvent 'duration (ly:make-duration 2 1)))
               (note 0 2)
               (dotted 1 1 1)))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &NOTE_HEADS_ENGRAVER,
        &REST_ENGRAVER,
        &DOTS_ENGRAVER,
        &DOT_COLUMN_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    let seen = eval(&mut vm, "(kept)");
    assert_eq!(show(&vm, seen), "()");
    let system = {
        let a = done.translation.arena.borrow();
        let score = a.children(GLOBAL)[0];
        let sym = vm.heap_mut().intern("rootSystem");
        a.get(score, sym)
    };
    let made = extract_grob_array(&mut vm, system, "all-elements").unwrap();
    let show_of = |vm: &mut Vm, text: &str, g: Value| {
        let f = eval(vm, text);
        let v = lily_engine_music::Host::call(vm, f, &[g]).unwrap();
        show(vm, v)
    };
    let got: Vec<String> = made
        .iter()
        .map(|&g| {
            let parts = [
                "(lambda (g) (let ((c (ly:grob-property g 'cause))) (if (ly:grob? c) c (if (null? c) c (car (ly:event-property c 'class))))))",
                "(lambda (g) (ly:grob? (ly:grob-object g 'dot)))",
                "(lambda (g) (let ((p (ly:grob-parent g 1))) (and p (eq? p (ly:grob-property g 'cause)))))",
                "(lambda (g) (let ((d (ly:grob-object g 'dots #f))) (if d (length (ly:grob-array->list d)) 0)))",
            ]
            .map(|p| show_of(&mut vm, p, g));
            format!("{} {}", show(&vm, g), parts.join(" "))
        })
        .collect();
    // Each: the grob, its cause, whether it has dots, whether its Y parent
    // is its cause, and the dots of a column.
    assert_eq!(
        got,
        [
            "#<Grob NoteHead > note-event #t #f 0",
            "#<Grob NoteHead > note-event #f #f 0",
            "#<Grob NoteHead > note-event #t #f 0",
            "#<Grob Rest > rest-event #t #f 0",
            "#<Grob Dots > #<Grob NoteHead > #f #t 0",
            "#<Grob DotColumn > () #f #f 3",
            "#<Grob Dots > #<Grob NoteHead > #f #t 0",
            "#<Grob Dots > #<Grob Rest > #f #t 0",
            // No dots, no column; a column of the next timestep's.
            "#<Grob NoteHead > note-event #f #f 0",
            "#<Grob NoteHead > note-event #t #f 0",
            "#<Grob Dots > #<Grob NoteHead > #f #t 0",
            "#<Grob DotColumn > () #f #f 1",
        ]
    );
    // A note head's dots take it as their X-offset; a rest's are moved
    // right by its width and the column's padding.
    let col = made[5];
    let rest_dots = made[7];
    vm.heap_mut().root(col);
    let f = eval(
        &mut vm,
        "(lambda (d c) (list (ly:grob-property-data d 'X-offset) (ly:grob-relative-coordinate d c 0)))",
    );
    let v = lily_engine_music::Host::call(&mut vm, f, &[rest_dots, col]).unwrap();
    assert_eq!(show(&vm, v), "(() 2.0)");
}

/// Global with the descriptions of the grobs that the Voice's note
/// engravers, `Rhythmic_column_engraver` and `Ledger_line_engraver` make, a
/// Score that consists `Paper_column_engraver`, a Staff that consists
/// `Ledger_line_engraver` and a Voice that consists `Note_heads_engraver`,
/// `Rest_engraver`, `Stem_engraver` and `Rhythmic_column_engraver`.
const NOTE_COLUMN_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface ledgered-interface note-head-interface
                                    rhythmic-head-interface)))
        (Rest (meta (name . Rest) (classes Item)
                    (interfaces grob-interface rest-interface rhythmic-head-interface)))
        (Stem (meta (name . Stem) (classes Item) (interfaces grob-interface stem-interface)))
        (StemStub (meta (name . StemStub) (classes Item) (interfaces grob-interface)))
        (Flag (meta (name . Flag) (classes Item) (interfaces grob-interface flag-interface)))
        (NoteColumn (axes 0 1)
                    (meta (name . NoteColumn) (classes Item)
                          (interfaces grob-interface axis-group-interface note-column-interface)))
        (LedgerLineSpanner (meta (name . LedgerLineSpanner) (classes Spanner)
                                 (interfaces grob-interface ledger-line-spanner-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (default-child Staff) (consists Paper_column_engraver))",
    "((context-name Staff) (translator-type Engraver_group) (accepts Voice)
      (default-child Voice) (consists Ledger_line_engraver))",
    "((context-name Voice) (translator-type Engraver_group) (consists Note_heads_engraver)
      (consists Rest_engraver) (consists Stem_engraver) (consists Rhythmic_column_engraver))",
];

/// `Rhythmic_column_engraver` and `Ledger_line_engraver` (MS3 step 16,
/// `rhythmic-column-engraver.cc`, `ledger-line-engraver.cc`), layout's
/// classes found before the music crate's placeholders: a NoteColumn of
/// each timestep with rhythmic heads, caused by the first, taking the heads
/// (a rest as its `rest`), the stem and the flag as its elements; one
/// LedgerLineSpanner of the Staff, with no cause, made at its first
/// timestep, bounded by the first and the last command columns, its
/// `note-heads` the ledgered heads but those whose `no-ledgers` is `#t`.
#[test]
fn note_columns_and_ledger_line_spanners_are_engraved_as_lilypond_does() {
    use lily_engine_layout::ledger_line_engraver::LEDGER_LINE_ENGRAVER;
    use lily_engine_layout::note_heads_engraver::NOTE_HEADS_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::rest_engraver::REST_ENGRAVER;
    use lily_engine_layout::rhythmic_column_engraver::RHYTHMIC_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::stem_engraver::STEM_ENGRAVER;
    use lily_engine_music::GLOBAL;
    let Some((mut vm, layout)) = vm_with(NOTE_COLUMN_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let music = eval(
        &mut vm,
        "(for-each (lambda (g) (set-object-property! g 'is-grob? #t)) '(NoteHead Stem))
         (set-object-property! 'no-ledgers 'backend-type? boolean?)
         (set-object-property! 'duration-log 'backend-type? integer?)
         (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
         (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
          (make-music 'ContextSpeccedMusic 'context-type 'Voice 'element
           (seq (make-music 'OverrideProperty 'symbol 'Stem 'grob-property-path '(duration-log)
                            'grob-value
                            (lambda (g)
                              (ly:duration-log
                               (ly:event-property (ly:grob-property (ly:grob-property g 'cause) 'cause)
                                                  'duration))))
                (par (note 0 2) (note 2 2))
                (note 1 3)
                (make-music 'OverrideProperty 'symbol 'NoteHead
                            'grob-property-path '(no-ledgers) 'grob-value #t)
                (note 3 2)
                (make-music 'RestEvent 'duration (ly:make-duration 2)))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &NOTE_HEADS_ENGRAVER,
        &REST_ENGRAVER,
        &STEM_ENGRAVER,
        &RHYTHMIC_COLUMN_ENGRAVER,
        &LEDGER_LINE_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    let seen = eval(&mut vm, "(kept)");
    assert_eq!(show(&vm, seen), "()");
    let system = {
        let a = done.translation.arena.borrow();
        let score = a.children(GLOBAL)[0];
        let sym = vm.heap_mut().intern("rootSystem");
        a.get(score, sym)
    };
    let made = extract_grob_array(&mut vm, system, "all-elements").unwrap();
    let show_of = |vm: &mut Vm, text: &str, g: Value| {
        let f = eval(vm, text);
        let v = lily_engine_music::Host::call(vm, f, &[g]).unwrap();
        show(vm, v)
    };
    let names = |l: &str| {
        format!(
            "(lambda (g) (map (lambda (x) (assq-ref (ly:grob-property x 'meta) 'name)) (ly:grob-array->list (or (ly:grob-object g '{l} #f) (ly:grob-list->grob-array '())))))"
        )
    };
    let made: Vec<Value> = made
        .into_iter()
        .filter(|&g| {
            let s = show(&vm, g);
            s.contains("NoteColumn") || s.contains("LedgerLineSpanner")
        })
        .collect();
    let got: Vec<String> = made
        .iter()
        .map(|&g| {
            let parts = [
                "(lambda (g) (let ((c (ly:grob-property g 'cause))) (if (ly:grob? c) c '())))",
                &names("note-heads"),
                "(lambda (g) (ly:grob? (ly:grob-object g 'rest)))",
                "(lambda (g) (ly:grob? (ly:grob-object g 'stem)))",
                &names("elements"),
                "(lambda (g) (if (ly:spanner? g)
                                 (map (lambda (d) (ly:grob-property (ly:spanner-bound g d) 'when)) '(-1 1))
                                 '()))",
            ]
            .map(|p| show_of(&mut vm, p, g));
            format!("{} {}", show(&vm, g), parts.join(" "))
        })
        .collect();
    // Each: the grob, its cause, its note heads, whether it has a rest and
    // a stem, its elements and a spanner's bounds' moments.
    assert_eq!(
        got,
        [
            "#<Grob LedgerLineSpanner > () (NoteHead NoteHead NoteHead) #f #f () (#<Mom 0> #<Mom 7/8>)",
            "#<Grob NoteColumn > #<Grob NoteHead > (NoteHead NoteHead) #f #t (NoteHead NoteHead Stem) ()",
            "#<Grob NoteColumn > #<Grob NoteHead > (NoteHead) #f #t (NoteHead Stem Flag) ()",
            // The note column takes the head whose no-ledgers is #t.
            "#<Grob NoteColumn > #<Grob NoteHead > (NoteHead) #f #t (NoteHead Stem) ()",
            "#<Grob NoteColumn > #<Grob Rest > () #t #t (Rest Stem) ()",
        ]
    );
}

/// [`NOTE_COLUMN_DEFS`] with StaffSymbol's description, and a Staff that
/// consists `Staff_symbol_engraver` and `Ledger_line_engraver`, its Voice
/// `Note_heads_engraver`.
const STAFF_SYMBOL_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface ledgered-interface note-head-interface
                                    rhythmic-head-interface)))
        (StaffSymbol (meta (name . StaffSymbol) (classes Spanner)
                           (interfaces grob-interface staff-symbol-interface)))
        (LedgerLineSpanner (meta (name . LedgerLineSpanner) (classes Spanner)
                                 (interfaces grob-interface ledger-line-spanner-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    NOTE_COLUMN_DEFS[1],
    "((context-name Staff) (translator-type Engraver_group) (accepts Voice)
      (default-child Voice) (consists Staff_symbol_engraver) (consists Ledger_line_engraver))",
    "((context-name Voice) (translator-type Engraver_group) (consists Note_heads_engraver))",
];

/// `Staff_symbol_engraver` (MS3 step 18, `staff-symbol-engraver.cc`),
/// layout's class found before the music crate's placeholder, with step
/// 16's `Ledger_line_engraver`: a StaffSymbol with no cause made at the
/// first timestep and at each `\startStaff`, bounded by the command columns
/// of its start and its `\stopStaff` (or the last), its own `staff-symbol`
/// and each grob's while it runs or at its stop; none at a timestep after
/// a stop; and the ledger line spanner ended and started again at each
/// staff symbol whose left bound is not its own, as LilyPond 2.26.0 gives
/// them (`$LILY_CORPUS/device-runs/2026-10-10-ms3-step18/session1-probe/`).
#[test]
fn staff_symbols_are_engraved_and_restart_the_ledger_lines_as_lilypond_does() {
    use lily_engine_layout::ledger_line_engraver::LEDGER_LINE_ENGRAVER;
    use lily_engine_layout::note_heads_engraver::NOTE_HEADS_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::staff_symbol_engraver::STAFF_SYMBOL_ENGRAVER;
    use lily_engine_music::GLOBAL;
    let Some((mut vm, layout)) = vm_with(STAFF_SYMBOL_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let music = eval(
        &mut vm,
        "(define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
         (define (staff d) (make-music 'StaffSpanEvent 'span-direction d))
         (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
          (make-music 'ContextSpeccedMusic 'context-type 'Voice 'element
           (seq (note 0 2)
                (par (staff 1) (note 1 2))
                (note 2 2)
                (par (staff -1) (note 3 2))
                (par (staff 1) (staff -1) (note 4 2))
                (par (make-music 'StaffSpanEvent) (note 5 2)))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &NOTE_HEADS_ENGRAVER,
        &LEDGER_LINE_ENGRAVER,
        &STAFF_SYMBOL_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    let seen = eval(&mut vm, "(kept)");
    assert_eq!(
        show(&vm, seen),
        r#"((programming-error "event span-direction is not set"))"#
    );
    let system = {
        let a = done.translation.arena.borrow();
        let score = a.children(GLOBAL)[0];
        let sym = vm.heap_mut().intern("rootSystem");
        a.get(score, sym)
    };
    let made = extract_grob_array(&mut vm, system, "all-elements").unwrap();
    let f = eval(
        &mut vm,
        "(lambda (g)
           (define (bounds s)
             (if (ly:spanner? s)
                 (map (lambda (d) (let ((c (ly:spanner-bound s d)))
                                    (if (ly:grob? c) (ly:grob-property c 'when) '())))
                      '(-1 1))
                 '()))
           (list (assq-ref (ly:grob-property g 'meta) 'name)
                 (let ((c (ly:grob-property g 'cause)))
                   (if (ly:stream-event? c) (car (ly:event-property c 'class)) c))
                 (bounds g)
                 (let ((s (ly:grob-object g 'staff-symbol #f)))
                   (cond ((eq? s g) 'itself) (s (bounds s)) (else #f)))))",
    );
    vm.heap_mut().root(f);
    let got: Vec<String> = made
        .into_iter()
        .map(|g| {
            let v = lily_engine_music::Host::call(&mut vm, f, &[g]).unwrap();
            show(&vm, v)
        })
        .filter(|s| !s.contains("PaperColumn"))
        .collect();
    // Each: its name, its cause, a spanner's bounds' moments and its staff
    // symbol's (or `itself`, or #f for none).
    let h = |s: &str| format!("(NoteHead note-event () {s})");
    assert_eq!(
        got,
        [
            h("(#<Mom 0> #<Mom 1/4>)"),
            "(StaffSymbol () (#<Mom 0> #<Mom 1/4>) itself)".to_string(),
            "(LedgerLineSpanner () (#<Mom 0> #<Mom 3/4>) (#<Mom 0> #<Mom 1/4>))".to_string(),
            // Stopped at 1/4: its heads still take it.
            h("(#<Mom 0> #<Mom 1/4>)"),
            // None at 1/2.
            h("#f"),
            // Started at 3/4, the ledger lines too.
            h("(#<Mom 3/4> #<Mom 1>)"),
            "(StaffSymbol () (#<Mom 3/4> #<Mom 1>) itself)".to_string(),
            "(LedgerLineSpanner () (#<Mom 3/4> #<Mom 1>) (#<Mom 3/4> #<Mom 1>))".to_string(),
            // Stopped and started at 1: the new one.
            h("(#<Mom 1> #<Mom 3/2>)"),
            "(StaffSymbol () (#<Mom 1> #<Mom 3/2>) itself)".to_string(),
            "(LedgerLineSpanner () (#<Mom 1> #<Mom 3/2>) (#<Mom 1> #<Mom 3/2>))".to_string(),
            h("(#<Mom 1> #<Mom 3/2>)"),
        ]
    );
}

/// Global with NoteHead's, Accidental's and AccidentalPlacement's
/// descriptions, and a Voice whose `Accidental_maker` makes them.
const ACCIDENTAL_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface rhythmic-head-interface)))
        (Accidental (meta (name . Accidental) (classes Item)
                          (interfaces grob-interface accidental-interface)))
        (AccidentalPlacement
         (meta (name . AccidentalPlacement) (classes Item)
               (interfaces grob-interface accidental-placement-interface))))))",
    DEFS[1],
    "((context-name Voice) (translator-type Engraver_group) (consists Accidental_maker))",
];

/// Part 3's function that accidentals are joined with (MS3 step 17), as
/// `accidental-placement.cc` gives it, on the grobs `Accidental_maker`
/// makes of three notes, c, d and c, which no engraver of layout's has
/// joined yet: of the note n, `(a 3n)` a NoteHead of the note, `(a 3n+1)`
/// an Accidental of the head and `(a 3n+2)` an AccidentalPlacement of the
/// accidental; `maker` the maker's handle.
#[test]
fn a_placement_takes_its_accidentals_by_notename_as_lilypond_does() {
    use lily_engine_layout::accidental_placement::add_accidental;
    let Some((mut vm, layout)) = vm_with(ACCIDENTAL_DEFS) else {
        return;
    };
    eval(
        &mut vm,
        "(define-public accidented '())
         (define-public maker #f)
         (ly:register-translator
           (lambda (context)
             (let ((heard #f))
               `((listeners (note-event . ,(lambda (t ev) (set! heard ev))))
                 (process-music
                  . ,(lambda (t)
                       (when heard
                         (let* ((head (ly:engraver-make-grob t 'NoteHead heard))
                                (acc (ly:engraver-make-grob t 'Accidental head))
                                (place (ly:engraver-make-grob t 'AccidentalPlacement acc)))
                           (set! maker t)
                           (set! accidented (append accidented (list head acc place))))
                         (set! heard #f)))))))
           'Accidental_maker)",
    );
    let (mut vm, _done) = glided_in(vm, layout, "(seq (note 0 2) (note 1 2) (note 0 2))");
    keep_diagnostics(&mut vm);
    eval(&mut vm, "(define (a n) (list-ref accidented n))");
    let g: Vec<Value> = (0..9).map(|n| eval(&mut vm, &format!("(a {n})"))).collect();
    let [h0, a0, p0, h1, a1, p1, h2, a2, _] = g[..] else {
        unreachable!()
    };
    let maker = eval(&mut vm, "maker");
    let kept = |vm: &mut Vm| {
        let k = eval(vm, "(kept)");
        show(vm, k)
    };
    // No Y parent, or one whose cause is no event: LilyPond's programming
    // error, unlocated, and nothing added.
    add_accidental(&mut vm, p0, a0, false, Value::FALSE).unwrap();
    set_parent(vm.heap(), a0, a1, Y);
    add_accidental(&mut vm, p0, a0, false, Value::FALSE).unwrap();
    assert_eq!(
        kept(&mut vm),
        "((programming-error \"note head has no event cause\") \
         (programming-error \"note head has no event cause\"))"
    );
    assert_eq!(get_parent(vm.heap(), a0, X), None);
    check(
        &mut vm,
        &[("(ly:grob-object (a 2) 'accidental-grobs)", "()")],
    );
    // Without staggering, keyed by the notename and 1: a new key in
    // front, an accidental in front of its key's; each made a child of
    // the placement on X.
    for (acc, head) in [(a0, h0), (a1, h1), (a2, h2)] {
        set_parent(vm.heap(), acc, head, Y);
        add_accidental(&mut vm, p0, acc, false, maker).unwrap();
        assert_eq!(get_parent(vm.heap(), acc, X), Some(p0));
    }
    assert_eq!(kept(&mut vm), "()");
    check(
        &mut vm,
        &[(
            "(equal? (ly:grob-object (a 2) 'accidental-grobs)
                     (list (list '(1 . 1) (a 4)) (list '(0 . 1) (a 7) (a 1))))",
            "#t",
        )],
    );
    // Staggered, keyed by the notename and the translator's number, an
    // exact integer of its own: the same notename apart for another
    // translator (the placement's handle stands for one).
    add_accidental(&mut vm, p1, a0, true, maker).unwrap();
    add_accidental(&mut vm, p1, a1, true, maker).unwrap();
    add_accidental(&mut vm, p1, a2, true, p0).unwrap();
    add_accidental(&mut vm, p1, a2, true, maker).unwrap();
    assert_eq!(get_parent(vm.heap(), a0, X), Some(p1));
    check(
        &mut vm,
        &[
            (
                "(map (lambda (e) (cons (car (car e)) (length (cdr e))))
                      (ly:grob-object (a 5) 'accidental-grobs))",
                "((0 . 1) (1 . 1) (0 . 2))",
            ),
            (
                "(let ((keys (map car (ly:grob-object (a 5) 'accidental-grobs))))
                   (list (map (lambda (k) (and (exact-integer? (cdr k)) (not (eqv? (cdr k) 1)))) keys)
                         (eqv? (cdr (car keys)) (cdr (caddr keys)))
                         (eqv? (cdr (cadr keys)) (cdr (caddr keys)))))",
                "((#t #t #t) #f #t)",
            ),
            (
                "(equal? (cdr (caddr (ly:grob-object (a 5) 'accidental-grobs))) (list (a 7) (a 1)))",
                "#t",
            ),
        ],
    );
}

/// Global with Clef's, ClefModifier's and the System's descriptions, and a
/// Staff that consists `Clef_engraver` with a treble clef, its notes in a
/// Voice.
const CLEF_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((Clef (meta (name . Clef) (classes Item) (interfaces grob-interface clef-interface)))
        (ClefModifier (meta (name . ClefModifier) (classes Item)
                            (interfaces grob-interface clef-modifier-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (default-child Staff))",
    "((context-name Staff) (translator-type Engraver_group) (consists Clef_engraver)
      (accepts Voice) (default-child Voice) (assign clefGlyph \"clefs.G\")
      (assign clefPosition -2) (assign clefTransposition 0) (assign firstClef #t)
      (assign explicitClefVisibility #(#f #t #t)))",
    "((context-name Voice) (translator-type Engraver_group))",
];

/// `Clef_engraver` (MS3 step 17, `clef-engraver.cc`), layout's class found
/// before the music crate's placeholder: a Clef at each timestep where a
/// break is allowed, its `glyph` pushed as `clefGlyph` and its
/// `staff-position` `clefPosition`; one where the clef's properties
/// changed or `forceClef` is `#t` even where a break is forbidden, marked
/// `non-default` and given `explicitClefVisibility`; the image's
/// `invalidate-alterations` called on the Staff and the Voice below it as
/// they change; a ClefModifier of a transposed clef, its text from
/// `clefTranspositionFormatter`, its direction the transposition's sign,
/// its parents the clef, which supports it; `forceClef` set to `'()`
/// where it is defined. The same rules gave LilyPond's and the engine's
/// clefs the same lines in `$LILY_CORPUS/device-runs/2026-10-09-ms3-step17/
/// session3/probe/`.
#[test]
fn clefs_are_engraved_as_lilypond_does() {
    use lily_engine_layout::clef_engraver::CLEF_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_music::GLOBAL;
    let Some((mut vm, layout)) = vm_with(CLEF_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let music = eval(
        &mut vm,
        "(for-each (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
                   (list (cons 'clefGlyph string?) (cons 'clefPosition number?)
                         (cons 'clefTransposition integer?) (cons 'clefTranspositionStyle symbol?)
                         (cons 'clefTranspositionFormatter procedure?)
                         (cons 'forbidBreak boolean?) (cons 'forceClef boolean?)
                         (cons 'firstClef boolean?) (cons 'explicitClefVisibility vector?)))
         (set-object-property! 'glyph 'backend-type? string?)
         (define-public invalidated '())
         (define-public (invalidate-alterations context)
           (set! invalidated (cons (ly:context-name context) invalidated)))
         (define (set context name value)
           (make-music 'ContextSpeccedMusic 'context-type context 'element
                       (make-music 'PropertySet 'symbol name 'value value)))
         (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
          (seq (set 'Staff 'clefTranspositionFormatter (lambda (n style) (list n style)))
               (set 'Staff 'clefTranspositionStyle 'plain)
               (note 0 2)
               (note 0 2)
               (set 'Staff 'forbidBreak #t)
               (note 0 2)
               (set 'Staff 'clefGlyph \"clefs.F\")
               (set 'Staff 'clefPosition 2)
               (set 'Staff 'clefTransposition -7)
               (note 0 2)
               (set 'Score 'forceClef #t)
               (note 0 2)
               (set 'Staff 'forbidBreak #f)
               (note 0 2)))",
    );
    vm.heap_mut().root(music);
    let classes = [&SCORE_ENGRAVER, &CLEF_ENGRAVER];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    let seen = eval(&mut vm, "(kept)");
    assert_eq!(show(&vm, seen), "()");
    // The Staff, then the Voice below it, at the first timestep, at the
    // change and where the clef is forced.
    assert_eq!(
        show_of(&mut vm, "(reverse invalidated)"),
        "(Staff Voice Staff Voice Staff Voice)"
    );
    let (system, force) = {
        let a = done.translation.arena.borrow();
        let score = a.children(GLOBAL)[0];
        let [system, force] = ["rootSystem", "forceClef"].map(|p| vm.heap_mut().intern(p));
        (a.get(score, system), a.here(score, force))
    };
    assert_eq!(force.map(|v| show(&vm, v)).as_deref(), Some("()"));
    let made = extract_grob_array(&mut vm, system, "all-elements").unwrap();
    let get = |vm: &mut Vm, g: Value, name: &str| {
        let f = eval(vm, "ly:grob-property");
        let sym = vm.heap_mut().intern(name);
        lily_engine_music::Host::call(vm, f, &[g, sym]).unwrap()
    };
    let got: Vec<String> = made
        .iter()
        .map(|&g| {
            let props = [
                "glyph",
                "staff-position",
                "non-default",
                "break-visibility",
                "text",
                "direction",
            ]
            .map(|p| {
                let v = get(&mut vm, g, p);
                show(&vm, v)
            });
            let parents = [X, Y].map(|a| get_parent(vm.heap(), g, a).map(|p| show(&vm, p)));
            let support = extract_grob_array(&mut vm, g, "side-support-elements")
                .map(|s| s.iter().map(|&s| show(&vm, s)).collect::<Vec<_>>());
            format!(
                "{} {} {parents:?} {support:?}",
                show(&vm, g),
                props.join(" ")
            )
        })
        .collect();
    let clef = |glyph: &str, pos: &str, non_default: &str, vis: &str| {
        format!(
            "#<Grob Clef > {glyph} {pos} {non_default} {vis} () () \
             [Some(\"#<Grob System >\"), Some(\"#<Grob System >\")] Ok([])"
        )
    };
    let modifier = "#<Grob ClefModifier > () () () () (\"8\" plain) -1 \
                    [Some(\"#<Grob Clef >\"), Some(\"#<Grob Clef >\")] Ok([\"#<Grob Clef >\"])";
    assert_eq!(
        got,
        [
            // The first clef, changed from #f; the next where a break is
            // allowed; none where it is forbidden.
            clef("\"clefs.G\"", "-2", "#t", "#(#f #t #t)"),
            clef("\"clefs.G\"", "-2", "()", "()"),
            // The changed clef, though a break is forbidden, and its
            // modifier, the formatter's text of the transposition's 8.
            clef("\"clefs.F\"", "2", "#t", "#(#f #t #t)"),
            modifier.to_string(),
            // Forced: a clef and its modifier again.
            clef("\"clefs.F\"", "2", "#t", "#(#f #t #t)"),
            modifier.to_string(),
            // Allowed again, at the last note and at the end: unchanged
            // clefs.
            clef("\"clefs.F\"", "2", "()", "()"),
            modifier.to_string(),
            clef("\"clefs.F\"", "2", "()", "()"),
            modifier.to_string(),
        ]
    );
}

/// Global with KeySignature's, KeyCancellation's, Clef's and the System's
/// descriptions, and a Staff that consists `Clef_engraver` and
/// `Key_engraver` (in LilyPond's order) with a treble clef and an order of
/// the alterations, its notes in a Voice.
const KEY_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((KeySignature (meta (name . KeySignature) (classes Item)
                            (interfaces grob-interface key-signature-interface)))
        (KeyCancellation (meta (name . KeyCancellation) (classes Item)
                               (interfaces grob-interface key-cancellation-interface)))
        (Clef (meta (name . Clef) (classes Item) (interfaces grob-interface clef-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (default-child Staff))",
    "((context-name Staff) (translator-type Engraver_group)
      (consists Clef_engraver) (consists Key_engraver)
      (accepts Voice) (default-child Voice) (assign clefGlyph \"clefs.G\")
      (assign clefPosition -2) (assign clefTransposition 0) (assign firstClef #t)
      (assign middleCClefPosition -6) (assign printKeyCancellation #t)
      (assign explicitKeySignatureVisibility #(#f #t #t))
      (assign keyAlterationOrder ((6 . -1/2) (2 . -1/2) (3 . 1/2) (0 . 1/2) (6 . 1/2))))",
    "((context-name Voice) (translator-type Engraver_group))",
];

/// `Key_engraver` (MS3 step 17, `key-engraver.cc`), layout's class found
/// before the music crate's placeholder: a key change read into
/// `keyAlterations` in `keyAlterationOrder`'s order, what the order lacks
/// after it with LilyPond's warning, and `tonic` (`:200`, `:201`); a
/// default KeySignature where a break is allowed, a non-default one at a
/// key change or where `keyAlterations` was set, with
/// `explicitKeySignatureVisibility`, and at a clef where
/// `createKeyOnClefChange` is set; its `alteration-alist` the key
/// reversed; a KeyCancellation of the alterations dropped or turned back,
/// where `printKeyCancellation` is set or the key is empty; and
/// `lastKeyAlterations` set to `keyAlterations` at each timestep's end
/// (`:158`). The 29 traces compare the three properties with LilyPond's.
#[test]
fn key_signatures_are_engraved_as_lilypond_does() {
    use lily_engine_layout::clef_engraver::CLEF_ENGRAVER;
    use lily_engine_layout::key_engraver::KEY_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_music::{GLOBAL, StreamEvent};
    let Some((mut vm, layout)) = vm_with(KEY_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let music = eval(
        &mut vm,
        "(for-each (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
                   (list (cons 'clefGlyph string?) (cons 'clefPosition number?)
                         (cons 'clefTransposition integer?) (cons 'firstClef boolean?)
                         (cons 'middleCClefPosition number?) (cons 'forbidBreak boolean?)
                         (cons 'printKeyCancellation boolean?) (cons 'keyAlterations list?)
                         (cons 'keyAlterationOrder list?) (cons 'createKeyOnClefChange boolean?)
                         (cons 'explicitKeySignatureVisibility vector?)))
         (set-object-property! 'glyph 'backend-type? string?)
         (define (set context name value)
           (make-music 'ContextSpeccedMusic 'context-type context 'element
                       (make-music 'PropertySet 'symbol name 'value value)))
         (define (key step alist)
           (make-music 'KeyChangeEvent 'tonic (ly:make-pitch -1 step 0) 'pitch-alist alist))
         (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
          (seq (note 0 2)
               (key 1 '((0 . 1/2) (1 . 0) (3 . 1/2)))
               (note 0 2)
               (set 'Staff 'forbidBreak #t)
               (note 0 2)
               (key 3 '((6 . -1/2) (0 . 0)))
               (note 0 2)
               (key 4 '((5 . 1/4) (6 . 1/2)))
               (note 0 2)
               (set 'Staff 'printKeyCancellation #f)
               (key 3 '((6 . -1/2)))
               (note 0 2)
               (key 0 '((0 . 0) (1 . 0)))
               (note 0 2)
               (set 'Staff 'keyAlterations '((2 . -1/2)))
               (note 0 2)
               (set 'Staff 'forbidBreak #f)
               (set 'Staff 'createKeyOnClefChange #t)
               (note 0 2)))",
    );
    vm.heap_mut().root(music);
    let classes = [&SCORE_ENGRAVER, &CLEF_ENGRAVER, &KEY_ENGRAVER];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    // The order lacks (5 . 1/4); the zero alterations it lacks are left out
    // without a warning.
    assert_eq!(
        show_of(&mut vm, "(kept)"),
        "((warning \"Incomplete keyAlterationOrder for key signature\"))"
    );
    // The Staff, removed at the end and kept in the arena.
    let (system, staff) = {
        let a = done.translation.arena.borrow();
        let score = a.children(GLOBAL)[0];
        let sym = vm.heap_mut().intern("rootSystem");
        let staff = (0..a.context_count() as u32)
            .find(|&c| show(&vm, a.name(vm.heap(), c)) == "Staff")
            .unwrap();
        (a.get(score, sym), staff)
    };
    let here: Vec<String> = ["keyAlterations", "lastKeyAlterations", "tonic"]
        .iter()
        .map(|p| {
            let sym = vm.heap_mut().intern(p);
            let v = done.translation.arena.borrow().here(staff, sym);
            v.map_or_else(|| "unset".into(), |v| show(&vm, v))
        })
        .collect();
    assert_eq!(here, ["((2 . -1/2))", "((2 . -1/2))", "#<Pitch c >"]);
    let made = extract_grob_array(&mut vm, system, "all-elements").unwrap();
    let get = |vm: &mut Vm, g: Value, name: &str| {
        let f = eval(vm, "ly:grob-property");
        let sym = vm.heap_mut().intern(name);
        lily_engine_music::Host::call(vm, f, &[g, sym]).unwrap()
    };
    let keys: Vec<Value> = made
        .into_iter()
        .filter(|&g| show(&vm, g) != "#<Grob Clef >")
        .collect();
    let got: Vec<String> = keys
        .iter()
        .map(|&g| {
            let props = [
                "alteration-alist",
                "c0-position",
                "non-default",
                "break-visibility",
            ]
            .map(|p| {
                let v = get(&mut vm, g, p);
                show(&vm, v)
            });
            let cause = get(&mut vm, g, "cause");
            let event = vm.heap().foreign::<StreamEvent>(cause).is_some();
            format!("{} {} {event}", show(&vm, g), props.join(" "))
        })
        .collect();
    let key = |alist: &str, non_default: bool, event: bool| {
        let (nd, vis) = if non_default {
            ("#t", "#(#f #t #t)")
        } else {
            ("()", "()")
        };
        format!("#<Grob KeySignature > {alist} -6 {nd} {vis} {event}")
    };
    let cancel = |alist: &str| format!("#<Grob KeyCancellation > {alist} -6 () () true");
    assert_eq!(
        got,
        [
            // A default key where a break is allowed.
            key("()", false, false),
            // D's alterations in the order's order, reversed; nothing to
            // cancel. None where a break is forbidden.
            key("((0 . 1/2) (3 . 1/2))", true, true),
            // F: its B flat, the sharps cancelled.
            key("((6 . -1/2))", true, true),
            cancel("((0 . 1/2) (3 . 1/2))"),
            // The order's B sharp, then what it lacks; the B flat turned
            // back.
            key("((5 . 1/4) (6 . 1/2))", true, true),
            cancel("((6 . -1/2))"),
            // No cancellation printed: none, though alterations dropped.
            key("((6 . -1/2))", true, true),
            // An empty key: cancelled though not printed.
            key("()", true, true),
            cancel("((6 . -1/2))"),
            // keyAlterations set: a non-default key with no cause.
            key("((2 . -1/2))", true, false),
            // A break allowed again and createKeyOnClefChange: non-default
            // at the clefs of the last note and of the end.
            key("((2 . -1/2))", true, false),
            key("((2 . -1/2))", true, false),
        ]
    );
}

/// Global with TimeSignature's and the System's descriptions, and a Staff
/// that consists `Time_signature_engraver` with a 4/4 `timeSignature`
/// (no Timing_translator: the test sets `timeSignature`,
/// `measurePosition` and `partialBusy` itself), its notes in a Voice.
const TIME_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((TimeSignature (meta (name . TimeSignature) (classes Item)
                             (interfaces grob-interface time-signature-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (default-child Staff))",
    "((context-name Staff) (translator-type Engraver_group)
      (consists Time_signature_engraver)
      (accepts Voice) (default-child Voice) (assign timeSignature (4 . 4))
      (assign initialTimeSignatureVisibility #(#f #t #t)))",
    "((context-name Voice) (translator-type Engraver_group))",
];

/// `Time_signature_engraver` (MS3 step 17, `time-signature-engraver.cc`),
/// layout's class found before the music crate's placeholder: a
/// TimeSignature where `timeSignature` is a pair or `#f` and not `eq?` to
/// the last one made of, its `time-signature` the spec unless an override
/// set one, the first one's `break-visibility`
/// `initialTimeSignatureVisibility`; its cause a `\polymetric` event whose
/// `time-signature` equals the context's before a `\time` one, a
/// `\polymetric` one of another time signature not heard; the warning of
/// one heard in mid-measure, but not under `partialBusy`.
#[test]
fn time_signatures_are_engraved_as_lilypond_does() {
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::time_signature_engraver::TIME_SIGNATURE_ENGRAVER;
    use lily_engine_music::GLOBAL;
    let Some((mut vm, layout)) = vm_with(TIME_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let music = eval(
        &mut vm,
        "(for-each (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
                   (list (cons 'timeSignature (lambda (x) (or (pair? x) (not x))))
                         (cons 'measurePosition ly:moment?) (cons 'partialBusy boolean?)
                         (cons 'initialTimeSignatureVisibility vector?)))
         (set-object-property! 'TimeSignature 'is-grob? #t)
         (set-object-property! 'time-signature 'backend-type? pair?)
         (define (in-staff m) (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element m))
         (define (set name value) (in-staff (make-music 'PropertySet 'symbol name 'value value)))
         ;; As `make-time-signature-set` broadcasts them, with a tag.
         (define (notify class spec tag)
           (make-music 'ApplyContext 'procedure
            (lambda (c)
              (ly:broadcast (ly:context-event-source c)
                            (ly:make-stream-event (ly:make-event-class class)
                                                  (list (cons 'time-signature spec)
                                                        (cons 'tag tag)))))))
         (define (time spec) (notify 'reference-time-signature-event spec 'time))
         (define (at position) (set 'measurePosition (ly:make-moment position)))
         (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
          (seq (at 0)
               (note 0 2)
               (note 0 2)
               (set 'timeSignature '(3 . 4))
               (time '(3 . 4))
               (note 0 2)
               (set 'timeSignature #f)
               (note 0 2)
               (set 'timeSignature '(2 . 4))
               (time '(2 . 4))
               (at 1/8)
               (note 0 2)
               (set 'timeSignature '(5 . 8))
               (time '(5 . 8))
               (set 'partialBusy #t)
               (note 0 2)
               (set 'partialBusy #f)
               (in-staff (make-music 'OverrideProperty 'symbol 'TimeSignature
                                     'grob-property-path '(time-signature)
                                     'grob-value '(6 . 8)))
               (set 'timeSignature '(7 . 8))
               (note 0 2)
               (in-staff (make-music 'RevertProperty 'symbol 'TimeSignature
                                     'grob-property-path '(time-signature)))
               (set 'timeSignature '(9 . 8))
               (notify 'polymetric-time-signature-event '(3 . 2) 'other)
               (note 0 2)
               (set 'timeSignature '(3 . 2))
               (notify 'polymetric-time-signature-event '(3 . 2) 'local)
               (time '(3 . 2))
               (at 0)
               (note 0 2)
               (set 'timeSignature (cons 3 2))
               (note 0 2)))",
    );
    vm.heap_mut().root(music);
    let classes = [&SCORE_ENGRAVER, &TIME_SIGNATURE_ENGRAVER];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    // At 1/8 by a `\time` only, and only without `partialBusy`.
    assert_eq!(
        show_of(&mut vm, "(kept)"),
        "((warning \"mid-measure time signature without \\\\partial\"))"
    );
    let system = {
        let a = done.translation.arena.borrow();
        let score = a.children(GLOBAL)[0];
        let sym = vm.heap_mut().intern("rootSystem");
        a.get(score, sym)
    };
    let made = extract_grob_array(&mut vm, system, "all-elements").unwrap();
    let get = |vm: &mut Vm, g: Value, name: &str| {
        let f = eval(vm, "ly:grob-property");
        let sym = vm.heap_mut().intern(name);
        lily_engine_music::Host::call(vm, f, &[g, sym]).unwrap()
    };
    let got: Vec<String> = made
        .iter()
        .map(|&g| {
            let props = ["time-signature", "break-visibility"].map(|p| {
                let v = get(&mut vm, g, p);
                show(&vm, v)
            });
            let cause = get(&mut vm, g, "cause");
            let tag = if cause == Value::NULL {
                "-".to_string()
            } else {
                let f = eval(&mut vm, "(lambda (ev) (ly:event-property ev 'tag))");
                let t = lily_engine_music::Host::call(&mut vm, f, &[cause]).unwrap();
                show(&vm, t)
            };
            format!("{} {} {tag}", show(&vm, g), props.join(" "))
        })
        .collect();
    let ts =
        |spec: &str, vis: &str, tag: &str| format!("#<Grob TimeSignature > {spec} {vis} {tag}");
    assert_eq!(
        got,
        [
            // The first, of the context's 4/4: no cause, the initial
            // visibility; none at the next note, the spec unchanged.
            ts("(4 . 4)", "#(#f #t #t)", "-"),
            ts("(3 . 4)", "()", "time"),
            // Senza misura.
            ts("#f", "()", "-"),
            // In mid-measure (warned), then under partialBusy.
            ts("(2 . 4)", "()", "time"),
            ts("(5 . 8)", "()", "time"),
            // The override's, not the spec.
            ts("(6 . 8)", "()", "-"),
            // A \polymetric of another time signature not heard: no cause
            // and no warning in mid-measure.
            ts("(9 . 8)", "()", "-"),
            // The \polymetric one before the \time one.
            ts("(3 . 2)", "()", "local"),
            // An equal spec that is not eq?: made again.
            ts("(3 . 2)", "()", "-"),
        ]
    );
}

/// Global with NoteHead's and the System's descriptions, a Staff that
/// consists `Accidental_engraver` and its Voice `Note_heads_engraver`.
const LOCAL_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface rhythmic-head-interface
                                    accidental-participating-head-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (default-child Staff))",
    "((context-name Staff) (translator-type Engraver_group) (consists Accidental_engraver)
      (accepts Voice) (default-child Voice))",
    "((context-name Voice) (translator-type Engraver_group) (consists Note_heads_engraver)
      (assign middleCPosition -6))",
];

/// `Accidental_engraver`'s translation half (MS3 step 17,
/// `accidental-engraver.cc`), layout's class found before the music
/// crate's placeholder: at a key change each context from the Staff down,
/// and the Score that holds one, given a copy of `keyAlterations` as
/// `localAlterations` (`:112`, `:125`); each head's pitch run through
/// `autoAccidentals`' rules from the image with the context its symbols
/// reach and the bar number, one less before the measure (`:171`), and
/// LilyPond's warnings of what is no rule; its alteration, bar number and
/// end put in front of `localAlterations` from the Voice up, the entry of
/// its key unlinked (`:429`); nothing of a head whose Voice sets
/// `nullAccidentals`. The calls and warnings are LilyPond 2.26.0's on the
/// same rules (`session6/probe/rules.ly` of step 17's runs).
#[test]
fn local_alterations_and_accidental_rules_are_kept_as_lilypond_does() {
    use lily_engine_layout::accidental_engraver::ACCIDENTAL_ENGRAVER;
    use lily_engine_layout::note_heads_engraver::NOTE_HEADS_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_music::GLOBAL;
    let Some((mut vm, layout)) = vm_with(LOCAL_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let music = eval(
        &mut vm,
        "(for-each (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
                   (list (cons 'middleCPosition number?) (cons 'autoAccidentals list?)
                         (cons 'keyAlterations list?) (cons 'localAlterations list?)
                         (cons 'internalBarNumber integer?) (cons 'measurePosition ly:moment?)
                         (cons 'nullAccidentals boolean?)))
         (define calls '())
         (define (rule tag)
           (lambda (c p bn)
             (set! calls (cons (list tag (ly:context-name c) p bn) calls))
             (cons #f #f)))
         (define (set context name value)
           (make-music 'ContextSpeccedMusic 'context-type context 'element
                       (make-music 'PropertySet 'symbol name 'value value)))
         (define (note* octave step alteration log)
           (make-music 'NoteEvent 'pitch (ly:make-pitch octave step alteration)
                       'duration (ly:make-duration log)))
         (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
         (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
          (seq (set 'Score 'localAlterations '())
               (set 'Score 'internalBarNumber 3)
               (set 'Score 'measurePosition (ly:make-moment -1/8))
               (set 'Staff 'autoAccidentals
                    (list 'Staff (rule 'a) 'Voice (rule 'b) 'Score (rule 'c)))
               (note* 0 0 1/2 3)
               (set 'Score 'measurePosition (ly:make-moment 0))
               (set 'Staff 'keyAlterations '((3 . 1/2) (0 . 1/2)))
               (note* 0 1 0 2)
               (par (note* 0 1 -1/2 2) (note* 0 0 0 2))
               (set 'Voice 'nullAccidentals #t)
               (note* 0 2 0 2)
               (set 'Voice 'nullAccidentals #f)
               (set 'Staff 'autoAccidentals (list \"x\" 'Staff (rule 'd) 3))
               (note* 0 3 0 2)))",
    );
    vm.heap_mut().root(music);
    let classes = [&SCORE_ENGRAVER, &ACCIDENTAL_ENGRAVER, &NOTE_HEADS_ENGRAVER];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(
        show_of(&mut vm, "(kept)"),
        "((warning \"accidental typesetting list must begin with context-name: x\") \
         (warning \"procedure or context-name expected for accidental rule, found \\\"x\\\"\") \
         (warning \"procedure or context-name expected for accidental rule, found 3\"))"
    );
    // A symbol moves to the context above of that name, if there is one
    // (no Voice above the Staff); the cis' before the measure's start, in
    // bar 2; the chord's heads in their order; no e', its Voice's
    // accidentals null.
    assert_eq!(
        show_of(&mut vm, "(reverse calls)"),
        "((a Staff #<Pitch cis' > 2) (b Staff #<Pitch cis' > 2) (c Score #<Pitch cis' > 2) \
         (a Staff #<Pitch d' > 3) (b Staff #<Pitch d' > 3) (c Score #<Pitch d' > 3) \
         (a Staff #<Pitch des' > 3) (b Staff #<Pitch des' > 3) (c Score #<Pitch des' > 3) \
         (a Staff #<Pitch c' > 3) (b Staff #<Pitch c' > 3) (c Score #<Pitch c' > 3) \
         (d Staff #<Pitch f' > 3))"
    );
    // The key's copies, d' then the chord (c' first, then des' unlinking
    // d''s entry), then f'; the same in the three contexts, Global none.
    let a = done.translation.arena.borrow();
    let sym = vm.heap_mut().intern("localAlterations");
    let here: Vec<String> = (0..a.context_count() as u32)
        .map(|c| {
            let v = a.here(c, sym);
            let name = show(&vm, a.name(vm.heap(), c));
            format!(
                "{name} {}",
                v.map_or_else(|| "unset".into(), |v| show(&vm, v))
            )
        })
        .collect();
    let local = "(((0 . 3) 0 3 . #<Mom 9/8>) ((0 . 1) -1/2 3 . #<Mom 5/8>) \
                 ((0 . 0) 0 3 . #<Mom 5/8>) (3 . 1/2) (0 . 1/2))";
    assert_eq!(a.parent(0), None);
    assert_eq!(GLOBAL, 0);
    assert_eq!(
        here,
        [
            "Global unset".into(),
            format!("Score {local}"),
            format!("Staff {local}"),
            format!("Voice {local}"),
        ]
    );
}

/// Global with the descriptions of the note heads, the accidentals, their
/// placement, the ties, their columns and the System; a Staff that consists
/// `Accidental_engraver`, its Voice `Note_heads_engraver` and
/// `Tie_engraver`.
const ACCIDENTAL_GROB_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface rhythmic-head-interface
                                    accidental-participating-head-interface)))
        (Accidental (meta (name . Accidental) (classes Item)
                          (interfaces grob-interface accidental-interface)))
        (AccidentalCautionary (meta (name . AccidentalCautionary) (classes Item)
                                    (interfaces grob-interface accidental-interface)))
        (AccidentalSuggestion (meta (name . AccidentalSuggestion) (classes Item)
                                    (interfaces grob-interface accidental-interface
                                                side-position-interface)))
        (AccidentalPlacement (meta (name . AccidentalPlacement) (classes Item)
                                   (interfaces grob-interface accidental-placement-interface)))
        (Tie (meta (name . Tie) (classes Spanner) (interfaces grob-interface tie-interface)))
        (TieColumn (meta (name . TieColumn) (classes Spanner)
                         (interfaces grob-interface tie-column-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (default-child Staff))",
    "((context-name Staff) (translator-type Engraver_group) (consists Accidental_engraver)
      (accepts Voice) (default-child Voice) (assign extraNatural #t))",
    "((context-name Voice) (translator-type Engraver_group) (consists Note_heads_engraver)
      (consists Tie_engraver) (assign middleCPosition -6))",
];

/// `Accidental_engraver`'s grobs (MS3 step 17, `accidental-engraver.cc`)
/// and `Tie_engraver`'s ties (`tie-engraver.cc`), layout's classes: each
/// accidental the rules ask for made through the head's engraver, in the
/// Voice, caused by the head and its child on Y (`:296`); one
/// `AccidentalPlacement` a timestep, in the Staff, holding them by
/// notename (`add_accidental`); `restore-first` of a natural restored
/// under `extraNatural`; a cautionary one `AccidentalCautionary` and
/// `forced`; an `AccidentalSuggestion` under `suggestAccidentals`, its
/// head its parent on X and its support. A tie from cis' to cis' bounded
/// by the heads, in a `TieColumn`, ends on the second head: its accidental
/// takes it as its `tie`, and `localAlterations` keeps the entry `tied`
/// (`:352–379`, `:413`); one from d' to e' is given up, LilyPond's
/// `unterminated tie`. LilyPond 2.26.0 gives these grobs of the same
/// music, their columns too (`session7/probe` of step 17's runs).
#[test]
fn accidentals_and_ties_are_engraved_as_lilypond_does() {
    use lily_engine_layout::accidental_engraver::ACCIDENTAL_ENGRAVER;
    use lily_engine_layout::note_heads_engraver::NOTE_HEADS_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::tie_engraver::TIE_ENGRAVER;
    use lily_engine_music::{GLOBAL, grob_of};
    let Some((mut vm, layout)) = vm_with(ACCIDENTAL_GROB_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let music = eval(
        &mut vm,
        "(for-each (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
                   (list (cons 'middleCPosition number?) (cons 'autoAccidentals list?)
                         (cons 'keyAlterations list?) (cons 'localAlterations list?)
                         (cons 'extraNatural boolean?) (cons 'suggestAccidentals boolean?)
                         (cons 'tieMelismaBusy boolean?)))
         (for-each (lambda (g) (set-object-property! g 'is-grob? #t))
                   '(Accidental AccidentalCautionary AccidentalSuggestion AccidentalPlacement
                     Tie TieColumn))
         (define (set context name value)
           (make-music 'ContextSpeccedMusic 'context-type context 'element
                       (make-music 'PropertySet 'symbol name 'value value)))
         (define (note* step alteration . arts)
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 step alteration)
                       'duration (ly:make-duration 2) 'articulations arts))
         (define (tie) (make-music 'TieEvent))
         (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
         (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
          (seq (set 'Staff 'localAlterations '())
               ;; An accidental of each altered pitch; a flat restores.
               (set 'Staff 'autoAccidentals
                    (list 'Staff
                          (lambda (c p bn)
                            (let ((alt (ly:pitch-alteration p)))
                              (cons (< alt 0) (not (= alt 0)))))))
               (note* 0 1/2 (tie))
               (note* 0 1/2)
               (par (note* 6 -1/2) (note* 1 1/2))
               (let ((m (note* 2 0)))
                 (ly:music-set-property! m 'cautionary #t)
                 (ly:music-set-property! m 'force-accidental #t)
                 m)
               (set 'Voice 'suggestAccidentals #t)
               (note* 3 1/2)
               (set 'Voice 'suggestAccidentals #f)
               (note* 1 0 (tie))
               (note* 2 0)))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &ACCIDENTAL_ENGRAVER,
        &NOTE_HEADS_ENGRAVER,
        &TIE_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(
        show_of(&mut vm, "(kept)"),
        "((warning \"unterminated tie\"))"
    );
    let system = {
        let a = done.translation.arena.borrow();
        let score = a.children(GLOBAL)[0];
        let sym = vm.heap_mut().intern("rootSystem");
        a.get(score, sym)
    };
    let made = extract_grob_array(&mut vm, system, "all-elements").unwrap();
    // Each grob but the heads: its name, where and by what it was made,
    // what caused it, and what the engravers set on it.
    let describe = eval(
        &mut vm,
        "(lambda (g)
           (define (name x)
             (cond ((ly:grob? x)
                    (let ((m (ly:grob-property x 'meta)))
                      (if (pair? m) (assq-ref m 'name) 'dead)))
                   ((ly:stream-event? x)
                    (let ((p (ly:event-property x 'pitch)))
                      (if (ly:pitch? p) (ly:pitch-notename p) (car (ly:event-property x 'class)))))
                   (else x)))
           (define (names a) (and (ly:grob-array? a) (map name (ly:grob-array->list a))))
           (define (cause x)
             (if (ly:grob? x)
                 (let ((c (ly:grob-property x 'cause)))
                   (if (ly:grob? c) (list (name c) (cause c)) (name c)))
                 x))
           (list (cause g)
                 (ly:grob-property g 'forced) (ly:grob-property g 'restore-first)
                 (name (ly:grob-object g 'tie #f)) (name (ly:grob-parent g 0))
                 (name (ly:grob-parent g 1))
                 (names (ly:grob-object g 'side-support-elements #f))
                 (let ((a (ly:grob-object g 'accidental-grobs #f)))
                   (and (pair? a) (map (lambda (e) (cons (car e) (map name (cdr e)))) a)))
                 (and (ly:spanner? g)
                      (map (lambda (d) (cause (ly:spanner-bound g d))) (list -1 1)))
                 (names (ly:grob-object g 'ties #f))))",
    );
    let places: Vec<(String, String)> = {
        let a = done.translation.arena.borrow();
        made.iter()
            .map(|&g| {
                let (_, id) = grob_of(vm.heap(), g).unwrap();
                let record = &a.records.grobs[id as usize];
                let origin = record.origin;
                let place = format!(
                    "{} {}",
                    show(&vm, a.name(vm.heap(), origin.0)),
                    a.translator_name(origin)
                );
                (show(&vm, record.name), place)
            })
            .collect()
    };
    let mut got = Vec::new();
    let mut heads = Vec::new();
    for (&g, (name, place)) in made.iter().zip(places) {
        if name == "NoteHead" {
            let f = eval(
                &mut vm,
                "(lambda (h) (ly:grob-object h 'accidental-grob #f))",
            );
            let acc = lily_engine_music::Host::call(&mut vm, f, &[g]).unwrap();
            heads.push(acc != Value::FALSE);
            continue;
        }
        let d = lily_engine_music::Host::call(&mut vm, describe, &[g])
            .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
        got.push(format!("{name} {place} {}", show(&vm, d)));
    }
    assert_eq!(
        got,
        [
            // From cis' to cis', in the column made at the second.
            "Tie Voice Tie_engraver (tie-event () () #f System TieColumn #f #f (0 0) #f)",
            "Accidental Voice Note_heads_engraver ((NoteHead 0) () () #f AccidentalPlacement \
             NoteHead #f #f #f #f)",
            "AccidentalPlacement Staff Accidental_engraver ((Accidental (NoteHead 0)) () () #f \
             System System #f (((0 . 1) Accidental)) #f #f)",
            "TieColumn Voice Tie_engraver ((Tie tie-event) () () #f NoteHead System #f #f (0 0) \
             (Tie))",
            // The tie ends on its head.
            "Accidental Voice Note_heads_engraver ((NoteHead 0) () () Tie AccidentalPlacement \
             NoteHead #f #f #f #f)",
            "AccidentalPlacement Staff Accidental_engraver ((Accidental (NoteHead 0)) () () #f \
             System System #f (((0 . 1) Accidental)) #f #f)",
            // The chord's: bes' restores a natural; one placement.
            "Accidental Voice Note_heads_engraver ((NoteHead 6) () #t #f AccidentalPlacement \
             NoteHead #f #f #f #f)",
            "AccidentalPlacement Staff Accidental_engraver ((Accidental (NoteHead 6)) () () #f \
             System System #f (((1 . 1) Accidental) ((6 . 1) Accidental)) #f #f)",
            "Accidental Voice Note_heads_engraver ((NoteHead 1) () () #f AccidentalPlacement \
             NoteHead #f #f #f #f)",
            "AccidentalCautionary Voice Note_heads_engraver ((NoteHead 2) #t () #f \
             AccidentalPlacement NoteHead #f #f #f #f)",
            "AccidentalPlacement Staff Accidental_engraver ((AccidentalCautionary (NoteHead 2)) () \
             () #f System System #f (((2 . 1) AccidentalCautionary)) #f #f)",
            "AccidentalSuggestion Voice Note_heads_engraver ((NoteHead 3) () () #f NoteHead System \
             (NoteHead) #f #f #f)",
            // From d', given up at e': dead, its cause kept.
            "Tie Voice Tie_engraver (tie-event () () #f System System #f #f (() ()) #f)",
        ]
    );
    // cis' cis' bes' dis' e' fis' d' e': the suggestion is no head's
    // accidental-grob.
    assert_eq!(heads, [true, true, true, true, true, false, false, false]);
    // The second cis', tied, kept as `tied`; then bes', fis', and the last
    // d' and e', which unlinked dis''s and the first e''s entries.
    let sym = vm.heap_mut().intern("localAlterations");
    let a = done.translation.arena.borrow();
    let voice = (0..a.context_count() as u32)
        .find(|&c| show(&vm, a.name(vm.heap(), c)) == "Voice")
        .unwrap();
    assert_eq!(
        show(&vm, a.here(voice, sym).unwrap()),
        "(((0 . 2) 0 0 . #<Mom 7/4>) ((0 . 1) 0 0 . #<Mom 3/2>) ((0 . 3) 1/2 0 . #<Mom 5/4>) \
         ((0 . 6) -1/2 0 . #<Mom 3/4>) ((0 . 0) tied 0 . #<Mom 1/2>))"
    );
}

/// Global with BarLine's and the System's descriptions, a Staff that
/// consists `Bar_engraver` and `Bar_probe`, with bar types of its own, and
/// its Voice.
const BAR_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((BarLine (glyph . \"|\") (glyph-right . #f)
                 (meta (name . BarLine) (classes Item)
                       (interfaces grob-interface bar-line-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (default-child Staff))",
    "((context-name Staff) (translator-type Engraver_group)
      (consists Bar_engraver) (consists Bar_probe) (accepts Voice) (default-child Voice)
      (assign measureBarType \"|\") (assign submeasureBarType \"!\")
      (assign startRepeatBarType \".|:\") (assign endRepeatBarType \":|.\")
      (assign doubleRepeatBarType \":..:\") (assign sectionBarType \"||\")
      (assign underlyingRepeatBarType \"||\") (assign segnoBarType \"S\")
      (assign caesuraType ()) (assign beatBase 1/4))",
    "((context-name Voice) (translator-type Engraver_group))",
];

/// The image's `bar-line::calc-glyph-name-for-direction` and what it
/// calls, from the installed `lily-library.scm` and `bar-line.scm`.
fn load_bar_glyph_names(vm: &mut Vm) {
    load_forms(
        vm,
        "lily-library.scm",
        &[
            "(define-public LEFT",
            "(define-public RIGHT",
            "(define-public CENTER",
            "(define-public assoc-get",
            "(define-public (!= ",
            "(define (index-cell",
        ],
    );
    load_forms(
        vm,
        "bar-line.scm",
        &[
            "(define annotation-char",
            "(define (strip-string-annotation",
            "(define-public (bar-line::calc-glyph-name-for-direction",
        ],
    );
}

/// `Bar_engraver`'s bar types (MS3 step 18, `bar-engraver.cc`), layout's
/// class found before the music crate's placeholder, with the image's
/// `bar-line::calc-glyph-name-for-direction` over a `bar-glyph-alist` of
/// the test's: none at the first timestep for a repeat or a default segno;
/// `whichBar`; a double repeat; trivial repeats unprinted; a caesura's
/// `bar-line` over the measure's; a segno over a section over the
/// underlying repeat; a caesura through `caesuraTypeTransform` with its
/// articulations; an ad hoc jump's underlying repeat; two caesuras' warnings;
/// a submeasure's division and the next timestep's none. Each BarLine
/// `currentBarLine` at its timestep's stop and none after, as LilyPond
/// 2.26.0 gives the probe's scores
/// (`$LILY_CORPUS/device-runs/2026-10-10-ms3-step18/session2-probe/`).
#[test]
fn bar_lines_take_their_bar_types_as_lilypond_does() {
    use lily_engine_layout::bar_engraver::BAR_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(BAR_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    load_bar_glyph_names(&mut vm);
    let music = eval(
        &mut vm,
        r#"(for-each
            (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
            `((measureBarType . ,string?) (submeasureBarType . ,string?)
              (startRepeatBarType . ,string?) (endRepeatBarType . ,string?)
              (doubleRepeatBarType . ,string?) (sectionBarType . ,string?)
              (underlyingRepeatBarType . ,string?) (segnoBarType . ,string?)
              (whichBar . ,string?) (caesuraType . ,list?) (caesuraTypeTransform . ,procedure?)
              (beatBase . ,rational?) (measureStartNow . ,boolean?) (repeatCommands . ,list?)
              (segnoStyle . ,symbol?) (submeasureBarsEnabled . ,boolean?)
              (submeasureStructure . ,list?) (measurePosition . ,ly:moment?)))
           (define-public bar-glyph-alist
             ;; As bar-line.scm's define-bar-line gives them.
             '(("|" . ("|" . #f)) ("||" . ("||" . #f)) (".|:" . ("|" . ".|:"))
               (":|." . (":|." . #f)) (":..:" . (":|." . ".|:")) ("S" . (#f . "S"))
               ("!" . ("!" . #f)) ("x" . ("x" . #f))))
           ;; `Bar_probe` writes, at each stop, the BarLines it acknowledged
           ;; and whether currentBarLine is each, or a stale one.
           (define-public probed '())
           (ly:register-translator
             (lambda (context)
               (let ((bars '()))
                 `((acknowledgers
                    (bar-line-interface . ,(lambda (t g s) (set! bars (cons g bars)))))
                   (stop-translation-timestep
                    . ,(lambda (t)
                         (let ((cur (ly:context-property context 'currentBarLine))
                               (now (ly:moment-main (ly:context-current-moment context))))
                           (for-each
                            (lambda (g)
                              (set! probed
                                    (cons (list now (ly:grob-property g 'glyph)
                                                (ly:grob-property g 'glyph-left)
                                                (ly:grob-property g 'glyph-right)
                                                (eq? cur g))
                                          probed)))
                            (reverse bars))
                           (when (and (null? bars) (ly:grob? cur))
                             (set! probed (cons (list now 'stale) probed)))
                           (set! bars '())))))))
             'Bar_probe)
           (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
           (define (st name value)
             (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
                         (make-music 'PropertySet 'symbol name 'value value)))
           (define (at . events) (apply par (note 0 2) events))
           (define (repeats . commands) (st 'repeatCommands commands))
           (define (caesura . arts) (make-music 'CaesuraEvent 'articulations arts))
           (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
            (make-music 'ContextSpeccedMusic 'context-type 'Voice 'element
             (seq (st 'measureStartNow #t)
                  (repeats '(start-repeat 2))
                  (at (make-music 'SegnoMarkEvent))
                  (st 'measureStartNow #f)
                  (repeats)
                  (st 'whichBar ".|:")
                  (at)
                  (st 'whichBar '())
                  (repeats '(end-repeat 1) '(start-repeat 2))
                  (at)
                  (repeats '(end-repeat 0) '(start-repeat 1))
                  (at)
                  (repeats)
                  (st 'measureStartNow #t)
                  (st 'caesuraType '((bar-line . "||") (underlying-bar-line . "x")))
                  (at (caesura))
                  (st 'measureStartNow #f)
                  (st 'segnoStyle 'bar-line)
                  (at (make-music 'SectionEvent) (make-music 'SegnoMarkEvent 'label 2))
                  (st 'caesuraType '((underlying-bar-line . ":|.")))
                  (st 'caesuraTypeTransform
                      (lambda (c type obs)
                        (if (pair? (assq-ref type 'articulations))
                            (acons 'bar-line "!" type)
                            type)))
                  (at (caesura (make-music 'ArticulationEvent 'articulation-type 'fermata)))
                  (at (make-music 'AdHocJumpEvent))
                  (st 'caesuraType '())
                  (st 'measureStartNow #t)
                  (at (caesura) (caesura (make-music 'ArticulationEvent 'articulation-type 'fermata)))
                  (st 'measureStartNow #f)
                  (st 'submeasureBarsEnabled #t)
                  (st 'submeasureStructure '(1 2))
                  (st 'measurePosition (ly:make-moment 1/4))
                  (at)
                  (st 'measurePosition (ly:make-moment 1/2))
                  (at))))"#,
    );
    vm.heap_mut().root(music);
    let classes = [&SCORE_ENGRAVER, &BAR_ENGRAVER];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(
        show_of(&mut vm, "(kept)"),
        "((warning \"conflict with event: `caesura-event'\") \
          (warning \"discarding event: `caesura-event'\"))"
    );
    assert_eq!(
        show_of(&mut vm, "(reverse probed)"),
        // At 0, nothing; at 3/4, trivial repeats; at 5/2, no division.
        "((1/4 \".|:\" \"|\" \".|:\" #t) \
          (1/2 \":..:\" \":|.\" \".|:\" #t) \
          (1 \"||\" \"||\" #f #t) \
          (5/4 \"S\" \"||\" \"S\" #t) \
          (3/2 \"!\" \"!\" #f #t) \
          (7/4 \"||\" \"||\" #f #t) \
          (2 \"|\" \"|\" #f #t) \
          (9/4 \"!\" \"!\" #f #t))"
    );
}

/// `ly:axis-group-interface::add-element` (MS3 step 18,
/// `axis-group-interface-scheme.cc:61–74`), which the image's
/// `Centered_bar_number_align_engraver` calls: `add_element` from Scheme,
/// the group the element's parent on its axes where it has none, its
/// `axis-group-parent-X` or `-Y` on each, then one of its `elements`; no
/// axes LilyPond's programming error, the element still added; an axis
/// that is no integer, and an argument that is no grob, Guile's type
/// errors. As `systems_and_paper_columns_are_made_as_lilypond_makes_them`
/// checks `add_element`.
#[test]
fn an_axis_group_takes_an_element_from_scheme_as_add_element_does() {
    let Some((mut vm, _done)) = glided_with(CLASSED_DEFS, "(seq (note 0 2) (note 1 2))") else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(
        &mut vm,
        "(define (caught thunk) (catch #t thunk (lambda (key . args) (list key (cadr args)))))
         (%grob-set-property-data! (g 3) 'axes '(1))
         (ly:grob-set-parent! (g 5) 1 (g 1))",
    );
    check(
        &mut vm,
        &[
            ("(ly:axis-group-interface::add-element (g 3) (g 4))", ""),
            ("(ly:axis-group-interface::add-element (g 3) (g 5))", ""),
            (
                "(map (lambda (e) (eq? (ly:grob-parent e 1) (g 3))) (list (g 4) (g 5)))",
                "(#t #f)",
            ),
            ("(null? (ly:grob-parent (g 4) 0))", "#t"),
            (
                "(map (lambda (e) (eq? (ly:grob-object e 'axis-group-parent-Y) (g 3))) (list (g 4) (g 5)))",
                "(#t #t)",
            ),
            ("(ly:grob-object (g 4) 'axis-group-parent-X)", "()"),
            (
                "(equal? (ly:grob-array->list (ly:grob-object (g 3) 'elements)) (list (g 4) (g 5)))",
                "#t",
            ),
            ("(ly:axis-group-interface::add-element (g 0) (g 2))", ""),
            (
                "(equal? (ly:grob-array->list (ly:grob-object (g 0) 'elements)) (list (g 2)))",
                "#t",
            ),
            ("(%grob-set-property-data! (g 3) 'axes '(x))", ""),
            (
                "(caught (lambda () (ly:axis-group-interface::add-element (g 3) (g 4))))",
                "(wrong-type-arg \"Wrong type (expecting ~A): ~S\")",
            ),
            (
                "(caught (lambda () (ly:axis-group-interface::add-element 1 (g 4))))",
                "(wrong-type-arg \"Wrong type argument in position ~A (expecting ~A): ~S\")",
            ),
        ],
    );
    assert_eq!(
        show_of(&mut vm, "(kept)"),
        "((programming-error \"axes should be nonempty\"))"
    );
}

/// Global with BarLine's, TextSpanner's, SpanBarStub's and the columns'
/// descriptions, a Score whose `Forbid_probe` writes `forbidBreak` as the
/// engravers see it at `process_music`, and a Voice whose `Spanner_probe`
/// starts and ends spanners and makes span bar stubs.
const BAR_BREAK_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((BarLine (meta (name . BarLine) (classes Item)
                       (interfaces grob-interface bar-line-interface)))
        (TextSpanner (meta (name . TextSpanner) (classes Spanner)
                           (interfaces grob-interface spanner-interface)))
        (SpanBarStub (meta (name . SpanBarStub) (classes Item)
                           (interfaces grob-interface span-bar-stub-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (default-child Staff) (consists Paper_column_engraver) (consists Forbid_probe)
      (assign forbidBreakBetweenBarLines #t))",
    "((context-name Staff) (translator-type Engraver_group) (consists Bar_engraver)
      (accepts Voice) (default-child Voice) (assign measureBarType \"|\"))",
    "((context-name Voice) (translator-type Engraver_group) (consists Spanner_probe))",
];

/// The rest of `Bar_engraver` (MS3 step 18, `bar-engraver.cc`):
/// `forbidBreak` set on the Score at `pre_process_music` where no bar line
/// is made and `forbidBreakBetweenBarLines` is `#t`, at the first timestep
/// too, and unset by `Paper_column_engraver` at the stop; the bar line the
/// right bound of each spanner ending at its timestep whose `to-barline`
/// is `#t`, the musical column staying that of the others; a span bar
/// stub's `allow-span-bar` and `allow-span-bar-above` `#f` where no bar
/// line is made. As LilyPond 2.26.0 gives the probe's scores
/// (`$LILY_CORPUS/device-runs/2026-10-10-ms3-step18/session3-probe/`).
#[test]
fn bar_lines_forbid_breaks_between_them_and_end_spanners_as_lilypond_does() {
    use lily_engine_layout::bar_engraver::BAR_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(BAR_BREAK_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    load_bar_glyph_names(&mut vm);
    let music = eval(
        &mut vm,
        r#"(for-each
            (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
            `((measureBarType . ,string?) (measureStartNow . ,boolean?)
              (forbidBreakBetweenBarLines . ,boolean?) (forbidBreak . ,boolean?)))
           (set-object-property! 'to-barline 'backend-type? boolean?)
           (define-public bar-glyph-alist '(("|" . ("|" . #f))))
           (define-public probed '())
           (define (now c) (ly:moment-main (ly:context-current-moment c)))
           (define (name g) (assq-ref (ly:grob-property g 'meta) 'name))
           (ly:register-translator
             (lambda (context)
               `((process-music
                  . ,(lambda (t)
                       (set! probed (cons (list (now context) 'forbid
                                                (ly:context-property context 'forbidBreak))
                                          probed))))))
             'Forbid_probe)
           ;; At each timestep: the spanners of the last ended, their right
           ;; bound the musical column, and three started, their
           ;; to-barline #t, #f and unset, and a SpanBarStub made; at the
           ;; stop, each ended spanner's to-barline and right bound, and
           ;; the stub's allow-span-bar and allow-span-bar-above.
           (ly:register-translator
             (lambda (context)
               (let ((open '()) (ended '()) (stub #f))
                 `((process-music
                    . ,(lambda (t)
                         (let ((col (ly:context-property context 'currentMusicalColumn)))
                           (for-each (lambda (sp)
                                       (ly:spanner-set-bound! sp RIGHT col)
                                       (ly:engraver-announce-end-grob t sp '()))
                                     open)
                           (set! ended open)
                           (set! open
                                 (map (lambda (tb)
                                        (let ((sp (ly:engraver-make-grob t 'TextSpanner '())))
                                          (if (boolean? tb) (ly:grob-set-property! sp 'to-barline tb))
                                          (ly:spanner-set-bound! sp LEFT col)
                                          sp))
                                      '(#t #f unset)))
                           (set! stub (ly:engraver-make-grob t 'SpanBarStub '())))))
                   (stop-translation-timestep
                    . ,(lambda (t)
                         (for-each (lambda (sp)
                                     (set! probed
                                           (cons (list (now context) (ly:grob-property sp 'to-barline)
                                                       (name (ly:spanner-bound sp RIGHT)))
                                                 probed)))
                                   ended)
                         (set! probed
                               (cons (list (now context) (ly:grob-property stub 'allow-span-bar)
                                           (ly:grob-property stub 'allow-span-bar-above))
                                     probed)))))))
             'Spanner_probe)
           (define (st context name value)
             (make-music 'ContextSpeccedMusic 'context-type context 'element
                         (make-music 'PropertySet 'symbol name 'value value)))
           (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
            (make-music 'ContextSpeccedMusic 'context-type 'Voice 'element
             (seq (note 0 2)
                  (note 1 2)
                  (st 'Staff 'measureStartNow #t)
                  (note 2 2)
                  (st 'Staff 'measureStartNow #f)
                  (st 'Score 'forbidBreakBetweenBarLines #f)
                  (note 3 2))))"#,
    );
    vm.heap_mut().root(music);
    let classes = [&SCORE_ENGRAVER, &PAPER_COLUMN_ENGRAVER, &BAR_ENGRAVER];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(show_of(&mut vm, "(kept)"), "()");
    assert_eq!(
        show_of(&mut vm, "(reverse probed)"),
        // Forbidden at 0 and 1/4, not at the bar line at 1/2 nor, with
        // forbidBreakBetweenBarLines #f, at 3/4 and at the end, 1; the
        // spanner of to-barline #t ending at 1/2 on the bar line; the
        // stubs' #f but at 1/2.
        "((0 forbid #t) (0 #f #f) \
          (1/4 forbid #t) (1/4 #t PaperColumn) (1/4 #f PaperColumn) (1/4 () PaperColumn) \
          (1/4 #f #f) \
          (1/2 forbid ()) (1/2 #t BarLine) (1/2 #f PaperColumn) (1/2 () PaperColumn) (1/2 () ()) \
          (3/4 forbid ()) (3/4 #t PaperColumn) (3/4 #f PaperColumn) (3/4 () PaperColumn) \
          (3/4 #f #f) \
          (1 forbid ()) (1 #t PaperColumn) (1 #f PaperColumn) (1 () PaperColumn) (1 #f #f))"
    );
}

/// Global with BarLine's, BarNumber's (all visible), CenteredBarNumber's
/// and the columns' descriptions; a Score with `Paper_column_engraver`,
/// `Bar_number_engraver` and `Number_probe`, which writes the bar numbers
/// at each stop and the centered ones' bounds at the end; a Staff with
/// `Bar_engraver`.
const BAR_NUMBER_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((BarLine (meta (name . BarLine) (classes Item)
                       (interfaces grob-interface bar-line-interface)))
        (BarNumber (break-visibility . #(#t #t #t))
                   (meta (name . BarNumber) (classes Item)
                         (interfaces grob-interface bar-number-interface)))
        (CenteredBarNumber (meta (name . CenteredBarNumber) (classes Spanner)
                                 (interfaces grob-interface centered-bar-number-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (default-child Staff) (consists Paper_column_engraver) (consists Bar_number_engraver)
      (consists Number_probe) (assign forbidBreakBetweenBarLines #t))",
    "((context-name Staff) (translator-type Engraver_group) (consists Bar_engraver)
      (accepts Voice) (default-child Voice) (assign measureBarType \"|\"))",
    "((context-name Voice) (translator-type Engraver_group))",
];

/// `Bar_number_engraver` (MS3 step 18, `bar-number-engraver.cc`), layout's
/// class found before the music crate's placeholder, with the image's
/// `all-bar-numbers-visible`,
/// `first-bar-number-invisible-and-no-parenthesized-bar-numbers` and
/// `robust-bar-number-function` (`translation-functions.scm`): a bar number
/// at the first timestep with no bar line, none where a break is forbidden
/// between bar lines, one at each bar line, one parenthesized where only a
/// break is allowed, its `break-visibility`'s middle `#f`; the
/// alternative's letter under `numbers-with-letters` and none under
/// `numbers`; none where the default visibility hides a mid-measure one;
/// centered ones from bar line to bar line, the last ended at a plain
/// one; `'()` as the text without a formatter; none where `currentBarNumber`
/// is unset; `stavesFound`'s grobs as `side-support-elements`, with
/// LilyPond's warning for what is no grob. As LilyPond 2.26.0 gives the
/// probe's scores (`$LILY_CORPUS/device-runs/2026-10-10-ms3-step18/session5-probe/`).
#[test]
fn bar_numbers_are_engraved_as_lilypond_does() {
    use lily_engine_layout::bar_engraver::BAR_ENGRAVER;
    use lily_engine_layout::bar_number_engraver::BAR_NUMBER_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(BAR_NUMBER_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    load_bar_glyph_names(&mut vm);
    load_forms(
        &mut vm,
        "translation-functions.scm",
        &[
            "(define-public (first-bar-number-invisible-and-no-parenthesized-bar-numbers",
            "(define-public (robust-bar-number-function",
            "(define-public (all-bar-numbers-visible",
        ],
    );
    let music = eval(
        &mut vm,
        r#"(for-each
            (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
            `((measureBarType . ,string?) (measureStartNow . ,boolean?)
              (forbidBreakBetweenBarLines . ,boolean?) (forbidBreak . ,boolean?)
              (currentBarNumber . ,integer?) (measurePosition . ,ly:moment?)
              (barNumberVisibility . ,procedure?) (barNumberFormatter . ,procedure?)
              (alternativeNumber . ,integer?) (alternativeNumberingStyle . ,symbol?)
              (centerBarNumbers . ,boolean?) (stavesFound . ,list?) (whichBar . ,string?)))
           (define-public bar-glyph-alist '(("|" . ("|" . #f))))
           (define-public probed '())
           (define (now c) (ly:moment-main (ly:context-current-moment c)))
           (define (when-of g) (if (ly:grob? g) (ly:moment-main (ly:grob-property g 'when)) g))
           (define (name g) (assq-ref (ly:grob-property g 'meta) 'name))
           ;; At each process-music, stavesFound the command column (and a
           ;; symbol at 1/2); at each stop, the bar numbers acknowledged; at
           ;; the end, each centered one's bounds.
           (ly:register-translator
             (lambda (context)
               (let ((made '()) (centered '()))
                 `((acknowledgers
                    (bar-number-interface . ,(lambda (t g s) (set! made (cons g made))))
                    (centered-bar-number-interface
                     . ,(lambda (t g s)
                          (set! made (cons g made))
                          (set! centered (cons g centered)))))
                   (process-music
                    . ,(lambda (t)
                         (let ((col (ly:context-property context 'currentCommandColumn)))
                           (ly:context-set-property! context 'stavesFound
                                                     (if (equal? (now context) 1/2)
                                                         (list col 'x)
                                                         (list col))))))
                   (stop-translation-timestep
                    . ,(lambda (t)
                         (for-each
                          (lambda (g)
                            (let ((side (ly:grob-object g 'side-support-elements)))
                              (set! probed
                                    (cons (list (now context) (name g) (ly:grob-property g 'text)
                                                (ly:grob-property g 'break-visibility)
                                                (if (ly:grob-array? side)
                                                    (ly:grob-array-length side)
                                                    side))
                                          probed))))
                          (reverse made))
                         (set! made '())))
                   (finalize
                    . ,(lambda (t)
                         (for-each
                          (lambda (g)
                            (set! probed
                                  (cons (list 'centered (ly:grob-property g 'text)
                                              (when-of (ly:spanner-bound g LEFT))
                                              (when-of (ly:spanner-bound g RIGHT)))
                                        probed)))
                          (reverse centered)))))))
             'Number_probe)
           (define (st name value)
             (make-music 'ContextSpeccedMusic 'context-type 'Score 'element
                         (make-music 'PropertySet 'symbol name 'value value)))
           ;; A timestep at the measure's start with the bar number, or
           ;; within one at the position.
           (define (bar n) (seq (st 'measureStartNow #t) (st 'currentBarNumber n)
                                (st 'measurePosition (ly:make-moment 0))))
           (define (within p) (seq (st 'measureStartNow #f)
                                   (st 'measurePosition (ly:make-moment p))))
           (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
            (make-music 'ContextSpeccedMusic 'context-type 'Voice 'element
             (seq (st 'barNumberVisibility all-bar-numbers-visible)
                  (st 'barNumberFormatter robust-bar-number-function)
                  (bar 1) (note 0 2)
                  (within 1/4) (note 1 2)
                  (bar 2) (note 2 2)
                  (within 1/4) (st 'forbidBreakBetweenBarLines #f) (note 3 2)
                  (st 'forbidBreakBetweenBarLines #t)
                  (st 'alternativeNumberingStyle 'numbers-with-letters)
                  (st 'alternativeNumber 2)
                  (bar 3) (note 4 2)
                  (st 'alternativeNumberingStyle 'numbers)
                  (bar 4) (note 5 2)
                  (st 'barNumberVisibility
                      first-bar-number-invisible-and-no-parenthesized-bar-numbers)
                  (within 1/4)
                  (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
                              (make-music 'PropertySet 'symbol 'whichBar 'value "|"))
                  (note 6 2)
                  (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
                              (make-music 'PropertyUnset 'symbol 'whichBar))
                  (st 'barNumberVisibility all-bar-numbers-visible)
                  (st 'centerBarNumbers #t)
                  (bar 5) (note 0 2)
                  (bar 6) (note 1 2)
                  (st 'centerBarNumbers #f)
                  (st 'barNumberFormatter #f)
                  (bar 7) (note 2 2)
                  (make-music 'ContextSpeccedMusic 'context-type 'Score 'element
                              (make-music 'PropertyUnset 'symbol 'currentBarNumber))
                  (st 'measureStartNow #t) (note 3 2))))"#,
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &BAR_ENGRAVER,
        &BAR_NUMBER_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(
        show_of(&mut vm, "(kept)"),
        "((warning \"ly:grob-list->grob-array encountered a non-grob object\"))"
    );
    assert_eq!(
        show_of(&mut vm, "(reverse probed)"),
        // None at 1/4, where a break is forbidden; none at 3/2, the bar
        // line mid-measure; none at 5/2, the bar number unset.
        "((0 BarNumber \"1\" #(#t #t #t) 1) \
          (1/2 BarNumber \"2\" #(#t #t #t) 1) \
          (3/4 BarNumber \"(2)\" #(#t #f #t) 1) \
          (1 BarNumber \"3b\" #(#t #t #t) 1) \
          (5/4 BarNumber \"4\" #(#t #t #t) 1) \
          (7/4 CenteredBarNumber \"5\" () ()) \
          (2 CenteredBarNumber \"6\" () ()) \
          (9/4 BarNumber () #(#t #t #t) 1) \
          (centered \"5\" 7/4 2) (centered \"6\" 2 9/4))"
    );
}

/// Global with StaffSymbol's and the four delimiters' descriptions, a Score
/// whose `System_start_delimiter_engraver` reads the hierarchy
/// `(SystemStartSquare (SystemStartBrace a b) c)` with `SystemStartBar` its
/// default, a StaffGroup whose own makes a `SystemStartBracket`, and a Staff
/// that consists `Staff_symbol_engraver`.
const DELIMITER_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((StaffSymbol (meta (name . StaffSymbol) (classes Spanner)
                           (interfaces grob-interface staff-symbol-interface)))
        (SystemStartBar (meta (name . SystemStartBar) (classes Spanner)
                              (interfaces grob-interface system-start-delimiter-interface)))
        (SystemStartBrace (meta (name . SystemStartBrace) (classes Spanner)
                                (interfaces grob-interface system-start-delimiter-interface)))
        (SystemStartBracket (meta (name . SystemStartBracket) (classes Spanner)
                                  (interfaces grob-interface system-start-delimiter-interface)))
        (SystemStartSquare (meta (name . SystemStartSquare) (classes Spanner)
                                 (interfaces grob-interface system-start-delimiter-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff) (accepts StaffGroup)
      (default-child Staff) (consists Paper_column_engraver)
      (consists System_start_delimiter_engraver) (assign systemStartDelimiter SystemStartBar)
      (assign systemStartDelimiterHierarchy (SystemStartSquare (SystemStartBrace a b) c)))",
    "((context-name StaffGroup) (translator-type Engraver_group) (accepts Staff)
      (default-child Staff) (consists System_start_delimiter_engraver)
      (assign systemStartDelimiter SystemStartBracket) (assign systemStartDelimiterHierarchy ()))",
    "((context-name Staff) (translator-type Engraver_group) (accepts Voice)
      (default-child Voice) (consists Staff_symbol_engraver))",
    "((context-name Voice) (translator-type Engraver_group))",
];

/// `System_start_delimiter_engraver` (MS3 step 18,
/// `system-start-delimiter-engraver.cc`), layout's class found before the
/// music crate's placeholder, with `Staff_symbol_engraver`, on a staff, a
/// StaffGroup of two and two staves more (five): the Score's SystemStartSquare
/// and SystemStartBrace of its hierarchy, from the first command column to
/// the last, with no cause; the staves taking the hierarchy's places depth
/// first, the fourth and fifth places added to the outer group; the StaffGroup's
/// SystemStartBracket of its two staves, supported by the Score's
/// delimiters it was acknowledged by; the inner delimiter supported by the
/// outer at finalize. A default type that is no symbol throws as Guile's
/// `symbol->string` does.
#[test]
fn system_start_delimiters_are_engraved_with_the_staves_they_span_as_lilypond_does() {
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::staff_symbol_engraver::STAFF_SYMBOL_ENGRAVER;
    use lily_engine_layout::system_start_delimiter_engraver::SYSTEM_START_DELIMITER_ENGRAVER;
    use lily_engine_music::GLOBAL;
    let Some((mut vm, layout)) = vm_with(DELIMITER_DEFS) else {
        return;
    };
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &STAFF_SYMBOL_ENGRAVER,
        &SYSTEM_START_DELIMITER_ENGRAVER,
    ];
    let music = eval(
        &mut vm,
        "(for-each (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
                   `((systemStartDelimiter . ,symbol?) (systemStartDelimiterHierarchy . ,list?)))
         (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
         (define (in type m) (make-music 'ContextSpeccedMusic 'context-type type 'element m))
         (define (new type m)
           (make-music 'ContextSpeccedMusic 'context-type type 'create-new #t 'element m))
         (define (staff) (new 'Staff (in 'Voice (seq (note 0 2) (note 1 2)))))
         (par (staff) (new 'StaffGroup (par (staff) (staff))) (staff) (staff))",
    );
    vm.heap_mut().root(music);
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    let system = {
        let a = done.translation.arena.borrow();
        let score = a.children(GLOBAL)[0];
        let sym = vm.heap_mut().intern("rootSystem");
        a.get(score, sym)
    };
    let made = extract_grob_array(&mut vm, system, "all-elements").unwrap();
    let all = vm.heap_mut().list(&made);
    vm.heap_mut().root(all);
    let f = eval(
        &mut vm,
        "(lambda (all)
           (define (name g) (assq-ref (ly:grob-property g 'meta) 'name))
           (define staves (filter (lambda (g) (eq? (name g) 'StaffSymbol)) all))
           (define (label g)
             (let ((s (memq g staves)))
               (if s (- (length staves) (length s)) (name g))))
           (define (labels g sym)
             (let ((a (ly:grob-object g sym #f)))
               (if a (map label (ly:grob-array->list a)) '())))
           (define (when s d) (ly:grob-property (ly:spanner-bound s d) 'when))
           (map (lambda (g)
                  (list (name g) (ly:grob-property g 'cause) (when g -1) (when g 1)
                        (labels g 'elements) (labels g 'side-support-elements)))
                (filter (lambda (g) (memq 'system-start-delimiter-interface
                                          (assq-ref (ly:grob-property g 'meta) 'interfaces)))
                        all)))",
    );
    let got = lily_engine_music::Host::call(&mut vm, f, &[all]).unwrap();
    // Each, in the order typeset (the StaffGroup's first): its name, its
    // cause, its bounds' moments, its elements (the staves by their order)
    // and its side-support-elements. The staves 0, 1 and 2 take the places
    // a, b and c, 3 and 4 places added to the outer group.
    assert_eq!(
        show(&vm, got),
        "((SystemStartBracket () #<Mom 0> #<Mom 1/2> (1 2) (SystemStartSquare SystemStartBrace)) \
         (SystemStartSquare () #<Mom 0> #<Mom 1/2> (0 1 2 3 4) ()) \
         (SystemStartBrace () #<Mom 0> #<Mom 1/2> (0 1) (SystemStartSquare)))"
    );

    let music = eval(
        &mut vm,
        "(seq (in 'Score (make-music 'PropertySet 'symbol 'systemStartDelimiterHierarchy 'value '()))
              (in 'Score (make-music 'PropertySet 'symbol 'systemStartDelimiter 'value #f))
              (staff))",
    );
    vm.heap_mut().root(music);
    let e = run_translator_with(&mut vm, music, layout, None, &classes)
        .err()
        .unwrap();
    assert_eq!(
        e.text(vm.heap()),
        "In procedure symbol->string: Wrong type argument in position 1 (expecting symbol): #f"
    );
}

/// Global with NoteHead's, Rest's, NoteColumn's, TextScript's (its
/// `direction` and `script-priority` LilyPond's) and the columns'
/// descriptions; a Score with `Paper_column_engraver`, and a Voice with
/// the heads', the rests', the columns' and the text scripts' engravers
/// and `Text_probe`.
const TEXT_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface rhythmic-head-interface)))
        (Rest (meta (name . Rest) (classes Item)
                    (interfaces grob-interface rest-interface rhythmic-head-interface)))
        (NoteColumn (axes 0 1)
                    (meta (name . NoteColumn) (classes Item)
                          (interfaces grob-interface axis-group-interface note-column-interface)))
        (TextScript (direction . -1) (script-priority . 200)
                    (meta (name . TextScript) (classes Item)
                          (interfaces grob-interface side-position-interface text-interface
                                      text-script-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice)
      (default-child Voice) (consists Paper_column_engraver))",
    "((context-name Voice) (translator-type Engraver_group) (consists Note_heads_engraver)
      (consists Rest_engraver) (consists Rhythmic_column_engraver) (consists Text_engraver)
      (consists Text_probe))",
];

/// `Text_engraver` (MS3 step 19, `text-engraver.cc`), layout's class found
/// before the music crate's placeholder: a TextScript of each text script
/// heard, its cause the event, its `text` the event's, its direction the
/// event's if it has one, its `script-priority` its own plus its place
/// among the timestep's, 200 for one that is no number; its X parent the
/// note column, or the rest where the column has no heads, and none for a
/// skip's, which `Paper_column_engraver` puts on its column only at the
/// Score's stop. As LilyPond 2.26.0 gives the probe's score
/// (`$LILY_CORPUS/device-runs/2026-10-10-ms3-step19/session2-probe/`), but
/// its markup's script, which is the event's text passed on as the
/// strings are.
#[test]
fn text_scripts_are_engraved_as_lilypond_does() {
    use lily_engine_layout::note_heads_engraver::NOTE_HEADS_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::rest_engraver::REST_ENGRAVER;
    use lily_engine_layout::rhythmic_column_engraver::RHYTHMIC_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::text_engraver::TEXT_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(TEXT_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let music = eval(
        &mut vm,
        "(set-object-property! 'TextScript 'is-grob? #t)
         (set-object-property! 'script-priority 'backend-type? number?)
         (define-public probed '())
         (define (name g) (if (ly:grob? g) (assq-ref (ly:grob-property g 'meta) 'name) g))
         ;; At each stop, each TextScript acknowledged: the moment, its
         ;; cause's class, text, direction and script-priority as set, and
         ;; its X parent's name.
         (ly:register-translator
           (lambda (context)
             (let ((made '()))
               `((acknowledgers
                  (text-script-interface . ,(lambda (t g s) (set! made (cons g made)))))
                 (stop-translation-timestep
                  . ,(lambda (t)
                       (for-each
                        (lambda (g)
                          (let ((cause (ly:grob-property g 'cause)))
                            (set! probed
                                  (cons (list (ly:moment-main (ly:context-current-moment context))
                                              (car (ly:event-property cause 'class))
                                              (ly:grob-property-data g 'text)
                                              (ly:grob-property-data g 'direction)
                                              (ly:grob-property-data g 'script-priority)
                                              (name (ly:grob-parent g 0)))
                                        probed))))
                        (reverse made))
                       (set! made '()))))))
           'Text_probe)
         (define (text t . dir)
           (if (null? dir)
               (make-music 'TextScriptEvent 'text t)
               (make-music 'TextScriptEvent 'text t 'direction (car dir))))
         (define (with m . scripts) (ly:music-set-property! m 'articulations scripts) m)
         (define (q) (ly:make-duration 2))
         (define (priority p)
           (make-music 'OverrideProperty 'symbol 'TextScript
                       'grob-property-path '(script-priority) 'grob-value p))
         (make-music 'ContextSpeccedMusic 'context-type 'Voice 'element
          (seq (with (note 0 2) (text \"up\" 1))
               (with (note 0 2) (text \"down\" -1) (text \"neutral\"))
               (with (make-music 'RestEvent 'duration (q)) (text \"rest\" 1))
               (with (make-music 'SkipEvent 'duration (q)) (text \"skip\" 1))
               (make-music 'EventChord 'elements
                           (list (note 0 2) (note 2 2) (text \"chord\" 1)))
               (priority 100)
               (with (note 0 2) (text \"a\" 1) (text \"b\" 1))
               (priority 1.5)
               (with (note 0 2) (text \"c\" 1) (text \"d\" -1))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &NOTE_HEADS_ENGRAVER,
        &REST_ENGRAVER,
        &RHYTHMIC_COLUMN_ENGRAVER,
        &TEXT_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(show_of(&mut vm, "(kept)"), "()");
    assert_eq!(
        show_of(&mut vm, "(reverse probed)"),
        "((0 text-script-event \"up\" 1 200 NoteColumn) \
          (1/4 text-script-event \"down\" -1 200 NoteColumn) \
          (1/4 text-script-event \"neutral\" -1 201 NoteColumn) \
          (1/2 text-script-event \"rest\" 1 200 Rest) \
          (3/4 text-script-event \"skip\" 1 200 ()) \
          (1 text-script-event \"chord\" 1 200 NoteColumn) \
          (5/4 text-script-event \"a\" 1 100 NoteColumn) \
          (5/4 text-script-event \"b\" 1 101 NoteColumn) \
          (3/2 text-script-event \"c\" 1 1.5 NoteColumn) \
          (3/2 text-script-event \"d\" -1 2.5 NoteColumn))"
    );
}

/// Global with NoteHead's, Accidental's, AccidentalPlacement's (its
/// `script-priority` LilyPond's), NoteColumn's, TextScript's (its
/// `direction`, `script-priority` and `side-axis` LilyPond's), ScriptRow's
/// and the columns' descriptions; a Score with `Paper_column_engraver` and
/// `Row_probe`, a Staff with the accidentals' and the script rows'
/// engravers, and a Voice with the heads', the columns' and the text
/// scripts' engravers.
const ROW_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface rhythmic-head-interface
                                    accidental-participating-head-interface)))
        (Accidental (meta (name . Accidental) (classes Item)
                          (interfaces grob-interface accidental-interface)))
        (AccidentalPlacement (script-priority . -100)
                             (meta (name . AccidentalPlacement) (classes Item)
                                   (interfaces grob-interface accidental-placement-interface)))
        (NoteColumn (axes 0 1)
                    (meta (name . NoteColumn) (classes Item)
                          (interfaces grob-interface axis-group-interface note-column-interface)))
        (TextScript (direction . -1) (script-priority . 200) (side-axis . 1)
                    (meta (name . TextScript) (classes Item)
                          (interfaces grob-interface side-position-interface text-interface
                                      text-script-interface)))
        (ScriptRow (meta (name . ScriptRow) (classes Item)
                         (interfaces grob-interface script-column-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (default-child Staff) (consists Paper_column_engraver) (consists Row_probe))",
    "((context-name Staff) (translator-type Engraver_group) (consists Accidental_engraver)
      (consists Script_row_engraver) (accepts Voice) (default-child Voice))",
    "((context-name Voice) (translator-type Engraver_group) (consists Note_heads_engraver)
      (consists Rhythmic_column_engraver) (consists Text_engraver) (assign middleCPosition -6))",
];

/// `Script_row_engraver` (MS3 step 19, `script-row-engraver.cc`), layout's
/// class found before the music crate's placeholder, with part 4's
/// `Script_column::add_side_positioned` and
/// `Side_position_interface::is_on_x_axis`: a ScriptRow once a timestep
/// has two scripts in the Staff, accidental placements or side positioned
/// items not non-musical (a non-musical text script on a skip is none, one
/// on a note column is), caused by the first; at the stop it takes the
/// placements and the scripts on the X axis whose `script-priority` is a
/// number, each given it as its `script-column`. A `side-axis` that is no
/// axis puts a script on neither, LilyPond's programming error if it has a
/// stencil. As LilyPond 2.26.0 gives the probe's score without
/// `Script_column_engraver`, which the engine does not have yet
/// (`$LILY_CORPUS/device-runs/2026-10-10-ms3-step19/session3-probe/`,
/// `probe-nocolumn.log`): its accidentals, each altered note's, are this
/// test's rule's.
#[test]
fn script_rows_take_the_placements_and_the_scripts_on_x_as_lilypond_does() {
    use lily_engine_layout::accidental_engraver::ACCIDENTAL_ENGRAVER;
    use lily_engine_layout::note_heads_engraver::NOTE_HEADS_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::rhythmic_column_engraver::RHYTHMIC_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::script_row_engraver::SCRIPT_ROW_ENGRAVER;
    use lily_engine_layout::text_engraver::TEXT_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(ROW_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let music = eval(
        &mut vm,
        "(for-each (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
                   (list (cons 'middleCPosition number?) (cons 'autoAccidentals list?)
                         (cons 'localAlterations list?)))
         (for-each (lambda (g) (set-object-property! g 'is-grob? #t))
                   '(Accidental AccidentalPlacement TextScript ScriptRow))
         (for-each (lambda (p) (set-object-property! (car p) 'backend-type? (cdr p)))
                   (list (cons 'script-priority number?) (cons 'side-axis number?)
                         (cons 'stencil ly:stencil?) (cons 'non-musical boolean?)))
         (define-public probed '())
         (define (name g) (if (ly:grob? g) (assq-ref (ly:grob-property g 'meta) 'name) g))
         ;; At each stop, after the Staff's, each ScriptRow acknowledged:
         ;; the moment, its name, its cause's, and each of its scripts'
         ;; name, script-priority and whether its script-column is the row.
         (ly:register-translator
           (lambda (context)
             (let ((made '()))
               `((acknowledgers
                  (script-column-interface . ,(lambda (t g s) (set! made (cons g made)))))
                 (stop-translation-timestep
                  . ,(lambda (t)
                       (for-each
                        (lambda (row)
                          (let ((scripts (ly:grob-object row 'scripts #f)))
                            (set! probed
                                  (cons (list (ly:moment-main (ly:context-current-moment context))
                                              (name row)
                                              (name (ly:grob-property row 'cause))
                                              (if (ly:grob-array? scripts)
                                                  (map (lambda (s)
                                                         (list (name s)
                                                               (ly:grob-property s 'script-priority)
                                                               (eq? (ly:grob-object s 'script-column)
                                                                    row)))
                                                       (ly:grob-array->list scripts))
                                                  scripts))
                                        probed))))
                        (reverse made))
                       (set! made '()))))))
           'Row_probe)
         (define (set context name value)
           (make-music 'ContextSpeccedMusic 'context-type context 'element
                       (make-music 'PropertySet 'symbol name 'value value)))
         (define (q) (ly:make-duration 2))
         (define (n step alteration . scripts)
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 step alteration) 'duration (q)
                       'articulations scripts))
         (define (s . scripts) (make-music 'SkipEvent 'duration (q) 'articulations scripts))
         (define (text t . dir)
           (if (null? dir)
               (make-music 'TextScriptEvent 'text t)
               (make-music 'TextScriptEvent 'text t 'direction (car dir))))
         ;; `\\override`'s music, which replaces an override of the property.
         (define (override grob prop value)
           (make-music 'OverrideProperty 'symbol grob 'grob-property-path (list prop)
                       'grob-value value 'pop-first #t))
         (define (revert grob prop)
           (make-music 'RevertProperty 'symbol grob 'grob-property-path (list prop)))
         (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
          (seq (set 'Staff 'localAlterations '())
               ;; An accidental of each altered note.
               (set 'Staff 'autoAccidentals
                    (list 'Staff
                          (lambda (c p bn) (cons #f (not (= (ly:pitch-alteration p) 0))))))
               (make-music 'ContextSpeccedMusic 'context-type 'Voice 'element
                (seq (n 0 0)
                     (n 0 0 (text \"a\" 1))
                     (n 0 0 (text \"a\" 1) (text \"b\" -1))
                     (n 0 1/2 (text \"a\" 1))
                     (n 0 1/2)
                     (s (text \"x\" 1) (text \"y\" -1))
                     (override 'TextScript 'side-axis 0)
                     (n 1 0 (text \"a\" 1) (text \"b\" -1))
                     (n 3 1/2 (text \"a\" 1) (text \"b\" -1))
                     (override 'TextScript 'stencil (ly:make-stencil '() '(0 . 0) '(0 . 0)))
                     (override 'TextScript 'side-axis 2)
                     (n 0 0 (text \"a\" 1) (text \"b\" -1))
                     (override 'TextScript 'stencil #f)
                     (n 0 0 (text \"a\" 1) (text \"b\" -1))
                     (revert 'TextScript 'stencil)
                     (revert 'TextScript 'side-axis)
                     (override 'TextScript 'non-musical #t)
                     (s (text \"x\" 1) (text \"y\" -1))
                     (n 0 0 (text \"x\" 1) (text \"y\" -1))
                     (revert 'TextScript 'non-musical)
                     (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
                                 (override 'AccidentalPlacement 'script-priority
                                           (lambda (g) #f)))
                     (override 'TextScript 'side-axis 0)
                     (n 0 1/2 (text \"a\" 1))))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &ACCIDENTAL_ENGRAVER,
        &NOTE_HEADS_ENGRAVER,
        &RHYTHMIC_COLUMN_ENGRAVER,
        &TEXT_ENGRAVER,
        &SCRIPT_ROW_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(
        show_of(&mut vm, "(kept)"),
        "((programming-error \"no side-axis setting found for grob TextScript.\") \
          (programming-error \"no side-axis setting found for grob TextScript.\"))"
    );
    assert_eq!(
        show_of(&mut vm, "(reverse probed)"),
        "((1/2 ScriptRow TextScript #f) \
          (3/4 ScriptRow TextScript ((AccidentalPlacement -100 #t))) \
          (5/4 ScriptRow TextScript #f) \
          (3/2 ScriptRow TextScript ((TextScript 200 #t) (TextScript 201 #t))) \
          (7/4 ScriptRow TextScript ((TextScript 200 #t) (TextScript 201 #t) \
                                     (AccidentalPlacement -100 #t))) \
          (2 ScriptRow TextScript #f) \
          (9/4 ScriptRow TextScript #f) \
          (11/4 ScriptRow TextScript #f) \
          (3 ScriptRow TextScript ((TextScript 200 #t))))"
    );
}

/// Global with Script's description (its `self-alignment-X`,
/// `slur-padding` and `staff-padding` LilyPond's), a Score and a Voice
/// with `Script_maker` and `Script_probe`, and the probe's
/// `scriptDefinitions`.
const SCRIPT_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((Script (self-alignment-X . 0) (slur-padding . 0.2) (staff-padding . 0.25)
                (meta (name . Script) (classes Item)
                      (interfaces grob-interface script-interface side-position-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice)
      (default-child Voice))",
    "((context-name Voice) (translator-type Engraver_group) (consists Script_maker)
      (consists Script_probe)
      (assign scriptDefinitions
              ((a . ((script-stencil . (feta . (\"staccato\" . \"staccato\")))
                     (padding . 0.2) (side-axis . 1) (script-priority . 10)))
               (b . ((padding . 0.5) (side-axis . 1)))
               (c . ((staff-padding . 0.5) (slur-padding . ()) (bogus-prop . 3)
                     (script-priority . 1.5)))
               (d . ((self-alignment-X . 1) (direction . 1) (font-size . -2))))))",
];

/// `Script_engraver::process_music` as far as `make_script_from_event`:
/// a Script of each articulation heard, caused by it, given the
/// articulation's properties with its place among the timestep's.
#[derive(Clone, Default)]
struct ScriptMaker {
    evs: Vec<Value>,
}

impl Work for ScriptMaker {
    fn listen(&mut self, _: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        self.evs.push(ev);
        Ok(())
    }
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for (i, &ev) in self.evs.iter().enumerate() {
            let id = tx.make_defined_grob("Script", Cause::Event(ev))?;
            let p = tx.grob_handle(id);
            let art_type = lily_engine_music::event_property(tx.heap(), ev, "articulation-type");
            make_script_from_event(tx, p, art_type, i)?;
        }
        Ok(())
    }
    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.evs.clear();
        Ok(())
    }
    fn values(&self, out: &mut Vec<Value>) {
        out.extend_from_slice(&self.evs);
    }
}

static SCRIPT_MAKER: TranslatorClass = TranslatorClass {
    name: "Script_maker",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: false,
    hears: &["articulation-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(ScriptMaker::default())),
    reads: &[],
    writes: &[],
};

/// `make_script_from_event` (MS3 step 42, part 4's, `script-engraver.cc`):
/// of each articulation, the properties of its `scriptDefinitions` entry
/// set on the Script but those whose value set on it already passes the
/// property's `backend-type?` and is not `'()` (`staff-padding`,
/// `self-alignment-X`), `'()` set over a definition's (`slur-padding`),
/// `script-priority` the entry's plus the place among the timestep's, or
/// the place; LilyPond's programming error for a property with no
/// `backend-type?` (`bogus-prop`, skipped) and for an articulation type
/// that is no symbol, its warning for one with no definition, which sets
/// nothing. As LilyPond 2.26.0 gives the probe's score through
/// `Script_engraver`
/// (`$LILY_CORPUS/device-runs/2026-10-20-ms3-step42/session1-probe/`,
/// `probe.ly`, `stderr.log`), its Scripts' `direction` an override's
/// procedure where LilyPond's is `ly:script-interface::calc-direction`.
#[test]
fn script_properties_are_set_from_the_definitions_as_lilypond_does() {
    let Some((mut vm, layout)) = vm_with(SCRIPT_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let music = eval(
        &mut vm,
        "(set-object-property! 'Script 'is-grob? #t)
         (set-object-property! 'scriptDefinitions 'translation-type? list?)
         (for-each (lambda (p) (set-object-property! (car p) 'backend-type? (cdr p)))
                   (list (cons 'script-stencil pair?) (cons 'padding number?)
                         (cons 'side-axis number?) (cons 'script-priority number?)
                         (cons 'staff-padding number?) (cons 'slur-padding number?)
                         (cons 'self-alignment-X number?) (cons 'direction ly:dir?)
                         (cons 'font-size number?)))
         (define keys '(script-stencil padding side-axis script-priority staff-padding
                        slur-padding bogus-prop self-alignment-X direction font-size))
         (define (shown v) (if (procedure? v) 'procedure v))
         (define-public probed '())
         ;; At each stop, each Script acknowledged: the moment, its cause's
         ;; articulation type and the property data of `keys`.
         (ly:register-translator
           (lambda (context)
             (let ((made '()))
               `((acknowledgers
                  (script-interface . ,(lambda (t g s) (set! made (cons g made)))))
                 (stop-translation-timestep
                  . ,(lambda (t)
                       (for-each
                        (lambda (g)
                          (set! probed
                                (cons (list (ly:moment-main (ly:context-current-moment context))
                                            (ly:event-property (ly:grob-property g 'cause)
                                                               'articulation-type)
                                            (map (lambda (k) (cons k (shown (ly:grob-property-data g k))))
                                                 keys))
                                      probed)))
                        (reverse made))
                       (set! made '()))))))
           'Script_probe)
         (define (art t)
           (let ((m (make-music 'ArticulationEvent)))
             (ly:music-set-property! m 'articulation-type t)
             m))
         (define (with m . scripts) (ly:music-set-property! m 'articulations scripts) m)
         (make-music 'ContextSpeccedMusic 'context-type 'Voice 'element
          (seq (make-music 'OverrideProperty 'symbol 'Script 'grob-property-path '(direction)
                           'grob-value (lambda (g) 0))
               (with (note 0 2) (art 'a) (art 'b) (art 'nope) (art 'c))
               (with (note 0 2) (art 'd) (art \"str\") (art 'a))))",
    );
    vm.heap_mut().root(music);
    let done = run_translator_with(&mut vm, music, layout, None, &[&SCRIPT_MAKER])
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(
        show_of(&mut vm, "(kept)"),
        "((warning \"do not know how to interpret articulation: nope\") \
          (programming-error \"invalid grob property name in script definition: bogus-prop\") \
          (programming-error \"articulation-type must be a symbol since 2.23.6: \\\"str\\\"\") \
          (warning \"do not know how to interpret articulation: \\\"str\\\"\"))"
    );
    assert_eq!(
        show_of(&mut vm, "(reverse probed)"),
        "((0 a ((script-stencil feta \"staccato\" . \"staccato\") (padding . 0.2) \
                (side-axis . 1) (script-priority . 10) (staff-padding . 0.25) \
                (slur-padding . 0.2) (bogus-prop) (self-alignment-X . 0) \
                (direction . procedure) (font-size))) \
          (0 b ((script-stencil) (padding . 0.5) (side-axis . 1) (script-priority . 1) \
                (staff-padding . 0.25) (slur-padding . 0.2) (bogus-prop) \
                (self-alignment-X . 0) (direction . procedure) (font-size))) \
          (0 nope ((script-stencil) (padding) (side-axis) (script-priority) \
                   (staff-padding . 0.25) (slur-padding . 0.2) (bogus-prop) \
                   (self-alignment-X . 0) (direction . procedure) (font-size))) \
          (0 c ((script-stencil) (padding) (side-axis) (script-priority . 4.5) \
                (staff-padding . 0.25) (slur-padding) (bogus-prop) (self-alignment-X . 0) \
                (direction . procedure) (font-size))) \
          (1/4 d ((script-stencil) (padding) (side-axis) (script-priority . 0) \
                  (staff-padding . 0.25) (slur-padding . 0.2) (bogus-prop) \
                  (self-alignment-X . 0) (direction . 1) (font-size . -2))) \
          (1/4 \"str\" ((script-stencil) (padding) (side-axis) (script-priority) \
                        (staff-padding . 0.25) (slur-padding . 0.2) (bogus-prop) \
                        (self-alignment-X . 0) (direction . procedure) (font-size))) \
          (1/4 a ((script-stencil feta \"staccato\" . \"staccato\") (padding . 0.2) \
                  (side-axis . 1) (script-priority . 12) (staff-padding . 0.25) \
                  (slur-padding . 0.2) (bogus-prop) (self-alignment-X . 0) \
                  (direction . procedure) (font-size))))"
    );
}

/// Global with NoteHead's, Rest's, NoteColumn's, DynamicText's, Hairpin's,
/// DynamicTextSpanner's (its `style` LilyPond's) and the columns'
/// descriptions; a Score with `Paper_column_engraver`, `Span_probe` and
/// the hairpin spanners engraver-init.ly's Score sets, and a Voice with
/// the heads', the rests', the columns' and the dynamics' engravers and
/// `Dynamic_probe`.
const DYNAMIC_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface rhythmic-head-interface)))
        (Rest (meta (name . Rest) (classes Item)
                    (interfaces grob-interface rest-interface rhythmic-head-interface)))
        (NoteColumn (axes 0 1)
                    (meta (name . NoteColumn) (classes Item)
                          (interfaces grob-interface axis-group-interface note-column-interface)))
        (DynamicText (meta (name . DynamicText) (classes Item)
                           (interfaces grob-interface dynamic-interface dynamic-text-interface
                                       text-interface)))
        (Hairpin (meta (name . Hairpin) (classes Spanner)
                       (interfaces grob-interface dynamic-interface hairpin-interface)))
        (DynamicTextSpanner (style . dashed-line)
                            (meta (name . DynamicTextSpanner) (classes Spanner)
                                  (interfaces grob-interface dynamic-interface
                                              dynamic-text-spanner-interface text-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice)
      (default-child Voice) (consists Paper_column_engraver) (consists Span_probe)
      (assign crescendoSpanner hairpin) (assign decrescendoSpanner hairpin))",
    "((context-name Voice) (translator-type Engraver_group) (consists Note_heads_engraver)
      (consists Rest_engraver) (consists Rhythmic_column_engraver) (consists Dynamic_engraver)
      (consists Dynamic_probe))",
];

/// `Dynamic_engraver` (MS3 step 19, `dynamic-engraver.cc`), layout's class
/// found before the music crate's placeholder: a DynamicText of each
/// absolute dynamic, its `text` the event's, its X parent the note column,
/// or the rest where it has no heads; a Hairpin of each crescendo or
/// decrescendo start, or a DynamicTextSpanner where the event's
/// `span-type` or the context's spanner is `text`, its `text` the event's
/// `span-text` or the context's text if that is a markup
/// (`Text_interface::is_markup`), `spanner-broken` where a
/// `\breakDynamicSpan` breaks it or its `style` is `none`; each spanner
/// ended by a stop, an absolute dynamic or a start, bounded by the
/// timestep's DynamicText, else its note column, else the musical column,
/// adjacent hairpins each other's `adjacent-spanners`; LilyPond's warnings
/// for an unknown style, a second dynamic and an unterminated spanner,
/// which is killed. As LilyPond 2.26.0 gives the probe's score without
/// `Bar_engraver`, which makes a bar line the right bound of a hairpin
/// that ends at one (`$LILY_CORPUS/device-runs/2026-10-10-ms3-step19/session4-probe/`,
/// `probe-nobar.log`; `probe.log` has `(BarLine 2)` and `(BarLine 4)`),
/// but its markup's text, which is written by its command's name.
#[test]
fn dynamics_and_hairpins_are_engraved_as_lilypond_does() {
    use lily_engine_layout::dynamic_engraver::DYNAMIC_ENGRAVER;
    use lily_engine_layout::note_heads_engraver::NOTE_HEADS_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::rest_engraver::REST_ENGRAVER;
    use lily_engine_layout::rhythmic_column_engraver::RHYTHMIC_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(DYNAMIC_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let music = eval(
        &mut vm,
        "(for-each (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
                   (list (cons 'crescendoSpanner symbol?) (cons 'decrescendoSpanner symbol?)
                         (cons 'crescendoText pair?)))
         (set-object-property! 'DynamicTextSpanner 'is-grob? #t)
         (set-object-property! 'style 'backend-type? symbol?)
         ;; The span dynamics' classes as define-event-classes.scm has them.
         (define-public (ly:make-event-class class)
           (if (memq class '(crescendo-event decrescendo-event))
               (list class 'span-dynamic-event)
               (list class)))
         ;; A markup command, as define-markup-commands.scm makes them.
         (define-public (markup-command-signature p) (procedure-property p 'markup-signature))
         (define-public (markup-list-function? p) #f)
         (define-public (line-markup layout props args) args)
         (define-public (italic-markup layout props arg) arg)
         (set-procedure-property! line-markup 'markup-signature (list list?))
         (set-procedure-property! italic-markup 'markup-signature (list string?))
         (define-public probed '())
         (define (name g) (if (ly:grob? g) (assq-ref (ly:grob-property g 'meta) 'name) g))
         (define (text t) (if (pair? t) (procedure-name (car t)) t))
         (define (bound g d)
           (let ((b (ly:spanner-bound g d)))
             (if (ly:grob? b)
                 (list (name b) (ly:moment-main (ly:grob-property (ly:item-get-column b) 'when)))
                 b)))
         ;; At each Voice stop, each DynamicText acknowledged: the moment,
         ;; its cause's class, text and X parent's name.
         (ly:register-translator
           (lambda (context)
             (let ((made '()))
               `((acknowledgers
                  (dynamic-text-interface . ,(lambda (t g s) (set! made (cons g made)))))
                 (stop-translation-timestep
                  . ,(lambda (t)
                       (for-each
                        (lambda (g)
                          (set! probed
                                (cons (list (ly:moment-main (ly:context-current-moment context))
                                            (car (ly:event-property (ly:grob-property g 'cause)
                                                                    'class))
                                            (ly:grob-property-data g 'text)
                                            (name (ly:grob-parent g 0)))
                                      probed)))
                        (reverse made))
                       (set! made '()))))))
           'Dynamic_probe)
         ;; At the Score's finalize, each spanner acknowledged: its name,
         ;; its cause's class, text and spanner-broken as set, its bounds'
         ;; names and their columns' moments, its adjacent-spanners' names;
         ;; a dead one as (dead <name>).
         (ly:register-translator
           (lambda (context)
             (let ((made '()))
               `((acknowledgers
                  (dynamic-interface
                   . ,(lambda (t g s)
                        (if (ly:spanner? g) (set! made (cons (cons g (name g)) made))))))
                 (finalize
                  . ,(lambda (t)
                       (for-each
                        (lambda (h)
                          (let ((g (car h)))
                            (set! probed
                                  (cons
                                   (if (null? (ly:grob-property g 'meta))
                                       (list 'dead (cdr h))
                                       (let ((adj (ly:grob-object g 'adjacent-spanners)))
                                         (list (name g)
                                               (car (ly:event-property (ly:grob-property g 'cause)
                                                                       'class))
                                               (text (ly:grob-property-data g 'text))
                                               (ly:grob-property-data g 'spanner-broken)
                                               (bound g -1)
                                               (bound g 1)
                                               (if (ly:grob-array? adj)
                                                   (map name (ly:grob-array->list adj))
                                                   adj))))
                                   probed))))
                        (reverse made)))))))
           'Span_probe)
         (define (q) (ly:make-duration 2))
         (define (n . arts)
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (q) 'articulations arts))
         (define (r . arts) (make-music 'RestEvent 'duration (q) 'articulations arts))
         (define (s . arts) (make-music 'SkipEvent 'duration (q) 'articulations arts))
         (define (dyn t) (make-music 'AbsoluteDynamicEvent 'text t))
         (define (cresc d . props) (apply make-music 'CrescendoEvent 'span-direction d props))
         (define (decresc d) (make-music 'DecrescendoEvent 'span-direction d))
         (define (brk) (make-music 'BreakDynamicSpanEvent))
         (define (set name value) (make-music 'PropertySet 'symbol name 'value value))
         (define (unset name) (make-music 'PropertyUnset 'symbol name))
         ;; `\\override`'s music, which replaces an override of the property.
         (define (override grob prop value)
           (make-music 'OverrideProperty 'symbol grob 'grob-property-path (list prop)
                       'grob-value value 'pop-first #t))
         (define (revert grob prop)
           (make-music 'RevertProperty 'symbol grob 'grob-property-path (list prop)))
         (make-music 'ContextSpeccedMusic 'context-type 'Voice 'element
          (seq (n (dyn \"p\"))
               (n (cresc -1)) (n) (n (cresc 1))
               (n (dyn \"ff\") (decresc -1)) (n (cresc 1) (dyn \"pp\"))
               (n (cresc -1)) (n (decresc -1)) (n (cresc 1))
               (r (dyn \"p\"))
               (s (cresc -1)) (s (cresc 1))
               (n (cresc -1)) (n (dyn \"f\"))
               (n (cresc -1)) (n (cresc -1)) (n (cresc 1))
               ;; \\crescTextCresc
               (set 'crescendoText (list line-markup (list (list italic-markup \"cresc.\"))))
               (set 'crescendoSpanner 'text)
               (n (cresc -1)) (n (cresc 1))
               ;; \\crescHairpin, then \\cresc and \\endcresc
               (unset 'crescendoText)
               (unset 'crescendoSpanner)
               (n (cresc -1 'span-type 'text 'span-text \"cresc.\")) (n (cresc 1))
               (n (cresc -1) (brk)) (n (cresc 1))
               (n (cresc -1)) (n (brk)) (n (cresc 1))
               (override 'DynamicTextSpanner 'style 'none)
               (n (cresc -1 'span-type 'text 'span-text \"cresc.\")) (n (cresc 1))
               (revert 'DynamicTextSpanner 'style)
               (n (cresc -1 'span-type 'text 'span-text 5)) (n (cresc 1))
               (set 'crescendoSpanner 'foo)
               (n (cresc -1)) (n (cresc 1))
               (unset 'crescendoSpanner)
               (make-music 'EventChord 'elements
                           (list (note 0 2) (note 2 2) (dyn \"p\") (dyn \"f\")))
               (n (decresc -1))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &NOTE_HEADS_ENGRAVER,
        &REST_ENGRAVER,
        &RHYTHMIC_COLUMN_ENGRAVER,
        &DYNAMIC_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(
        show_of(&mut vm, "(kept)"),
        "((warning \"unknown crescendo style: foo\\ndefaulting to hairpin.\") \
          (warning \"conflict with event: `absolute-dynamic-event'\") \
          (warning \"discarding event: `absolute-dynamic-event'\") \
          (warning \"unterminated decrescendo\"))"
    );
    assert_eq!(
        show_of(&mut vm, "(reverse probed)"),
        "((0 absolute-dynamic-event \"p\" NoteColumn) \
          (1 absolute-dynamic-event \"ff\" NoteColumn) \
          (5/4 absolute-dynamic-event \"pp\" NoteColumn) \
          (9/4 absolute-dynamic-event \"p\" Rest) \
          (13/4 absolute-dynamic-event \"f\" NoteColumn) \
          (8 absolute-dynamic-event \"p\" NoteColumn) \
          (Hairpin crescendo-event () () (NoteColumn 1/4) (NoteColumn 3/4) ()) \
          (Hairpin decrescendo-event () () (DynamicText 1) (DynamicText 5/4) ()) \
          (Hairpin crescendo-event () () (NoteColumn 3/2) (NoteColumn 7/4) (Hairpin)) \
          (Hairpin decrescendo-event () () (NoteColumn 7/4) (NoteColumn 2) (Hairpin)) \
          (Hairpin crescendo-event () () (PaperColumn 5/2) (PaperColumn 11/4) ()) \
          (Hairpin crescendo-event () () (NoteColumn 3) (DynamicText 13/4) ()) \
          (Hairpin crescendo-event () () (NoteColumn 7/2) (NoteColumn 15/4) (Hairpin)) \
          (Hairpin crescendo-event () () (NoteColumn 15/4) (NoteColumn 4) (Hairpin)) \
          (DynamicTextSpanner crescendo-event line-markup () (NoteColumn 17/4) (NoteColumn 9/2) ()) \
          (DynamicTextSpanner crescendo-event \"cresc.\" () (NoteColumn 19/4) (NoteColumn 5) ()) \
          (Hairpin crescendo-event () #t (NoteColumn 21/4) (NoteColumn 11/2) ()) \
          (Hairpin crescendo-event () #t (NoteColumn 23/4) (NoteColumn 25/4) ()) \
          (DynamicTextSpanner crescendo-event \"cresc.\" #t (NoteColumn 13/2) (NoteColumn 27/4) ()) \
          (DynamicTextSpanner crescendo-event () () (NoteColumn 7) (NoteColumn 29/4) ()) \
          (Hairpin crescendo-event () () (NoteColumn 15/2) (NoteColumn 31/4) ()) \
          (dead Hairpin))"
    );
}

/// Global with NoteHead's, Rest's, Stem's, StemStub's, Flag's,
/// NoteColumn's, DynamicText's, Hairpin's, DynamicTextSpanner's,
/// DynamicLineSpanner's (its `axes` and `direction` LilyPond's),
/// FootnoteProbe's (`footnote-spanner-interface`, which no grob of
/// LilyPond's has) and the columns' descriptions; a Score with
/// `Paper_column_engraver`, `Line_probe` and the hairpin spanners
/// engraver-init.ly's Score sets, and a Voice with the heads', the rests',
/// the stems', the columns' and the dynamics' engravers and
/// `Footnote_probe`.
const DYNAMIC_ALIGN_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface rhythmic-head-interface)))
        (Rest (meta (name . Rest) (classes Item)
                    (interfaces grob-interface rest-interface rhythmic-head-interface)))
        (Stem (meta (name . Stem) (classes Item) (interfaces grob-interface stem-interface)))
        (StemStub (meta (name . StemStub) (classes Item) (interfaces grob-interface)))
        (Flag (meta (name . Flag) (classes Item) (interfaces grob-interface flag-interface)))
        (NoteColumn (axes 0 1)
                    (meta (name . NoteColumn) (classes Item)
                          (interfaces grob-interface axis-group-interface note-column-interface)))
        (DynamicText (meta (name . DynamicText) (classes Item)
                           (interfaces grob-interface dynamic-interface dynamic-text-interface
                                       text-interface)))
        (Hairpin (meta (name . Hairpin) (classes Spanner)
                       (interfaces grob-interface dynamic-interface hairpin-interface)))
        (DynamicTextSpanner (style . dashed-line)
                            (meta (name . DynamicTextSpanner) (classes Spanner)
                                  (interfaces grob-interface dynamic-interface
                                              dynamic-text-spanner-interface text-interface)))
        (DynamicLineSpanner (axes 1) (direction . -1)
                            (meta (name . DynamicLineSpanner) (classes Spanner)
                                  (interfaces grob-interface axis-group-interface dynamic-interface
                                              dynamic-line-spanner-interface
                                              outside-staff-interface side-position-interface)))
        (FootnoteProbe (meta (name . FootnoteProbe) (classes Item)
                             (interfaces grob-interface footnote-spanner-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice)
      (default-child Voice) (consists Paper_column_engraver) (consists Line_probe)
      (assign crescendoSpanner hairpin) (assign decrescendoSpanner hairpin))",
    "((context-name Voice) (translator-type Engraver_group) (consists Note_heads_engraver)
      (consists Rest_engraver) (consists Stem_engraver) (consists Rhythmic_column_engraver)
      (consists Dynamic_engraver) (consists Dynamic_align_engraver) (consists Footnote_probe))",
];

/// `Dynamic_align_engraver` (MS3 step 19, `dynamic-align-engraver.cc`),
/// layout's class found before the music crate's placeholder: a
/// DynamicLineSpanner caused by a timestep's first dynamic, the dynamics
/// its elements until no spanner runs; an explicit direction not the
/// line's, or the end of a spanner `\breakDynamicSpan` broke, ends it and
/// starts another; its bounds the timestep's first DynamicText, else its
/// first started or ended spanner's; its support the rhythmic heads and
/// stems of its timesteps, but those of a broken spanner (`style` `none`);
/// a footnote spanner whose Y parent is a dynamic its element. As LilyPond
/// 2.26.0 gives the probe's score
/// (`$LILY_CORPUS/device-runs/2026-10-10-ms3-step19/session5-probe/`,
/// `probe.log`).
#[test]
fn dynamic_line_spanners_align_the_dynamics_as_lilypond_does() {
    use lily_engine_layout::dynamic_align_engraver::DYNAMIC_ALIGN_ENGRAVER;
    use lily_engine_layout::dynamic_engraver::DYNAMIC_ENGRAVER;
    use lily_engine_layout::note_heads_engraver::NOTE_HEADS_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::rest_engraver::REST_ENGRAVER;
    use lily_engine_layout::rhythmic_column_engraver::RHYTHMIC_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::stem_engraver::STEM_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(DYNAMIC_ALIGN_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    let music = eval(
        &mut vm,
        "(for-each (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
                   (list (cons 'crescendoSpanner symbol?) (cons 'decrescendoSpanner symbol?)))
         (for-each (lambda (g) (set-object-property! g 'is-grob? #t))
                   '(DynamicTextSpanner FootnoteProbe))
         (set-object-property! 'style 'backend-type? symbol?)
         (set-object-property! 'Stem 'is-grob? #t)
         (set-object-property! 'duration-log 'backend-type? integer?)
         ;; The span dynamics' classes as define-event-classes.scm has them.
         (define-public (ly:make-event-class class)
           (if (memq class '(crescendo-event decrescendo-event))
               (list class 'span-dynamic-event)
               (list class)))
         (define-public probed '())
         (define (name g) (if (ly:grob? g) (assq-ref (ly:grob-property g 'meta) 'name) g))
         (define (at g)
           (let ((c (if (ly:spanner? g) (ly:spanner-bound g -1) g)))
             (if (ly:grob? c)
                 (list (name g) (ly:moment-main (ly:grob-property (ly:item-get-column c) 'when)))
                 (list (name g) c))))
         (define (bound g d)
           (let ((b (ly:spanner-bound g d)))
             (if (ly:grob? b) (at b) b)))
         (define (arr g sym)
           (let ((a (ly:grob-object g sym)))
             (if (ly:grob-array? a) (map at (ly:grob-array->list a)) a)))
         ;; A FootnoteProbe at each DynamicText whose text is \"fn\", its Y
         ;; parent the DynamicText.
         (ly:register-translator
           (lambda (context)
             `((acknowledgers
                (dynamic-text-interface
                 . ,(lambda (t g s)
                      (if (equal? (ly:grob-property g 'text) \"fn\")
                          (ly:grob-set-parent! (ly:engraver-make-grob t 'FootnoteProbe g)
                                               1 g)))))))
           'Footnote_probe)
         ;; At the Score's finalize, each DynamicLineSpanner acknowledged:
         ;; its cause's name, its direction as set, its bounds' names and
         ;; their columns' moments, its elements' and its
         ;; side-support-elements' the same.
         (ly:register-translator
           (lambda (context)
             (let ((made '()))
               `((acknowledgers
                  (dynamic-line-spanner-interface . ,(lambda (t g s) (set! made (cons g made)))))
                 (finalize
                  . ,(lambda (t)
                       (for-each
                        (lambda (g)
                          (set! probed
                                (cons (list (name (ly:grob-property g 'cause))
                                            (ly:grob-property-data g 'direction)
                                            (bound g -1)
                                            (bound g 1)
                                            (arr g 'elements)
                                            (arr g 'side-support-elements))
                                      probed)))
                        (reverse made)))))))
           'Line_probe)
         (define (n . arts)
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration 2)
                       'articulations arts))
         (define (e . arts)
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration 3)
                       'articulations arts))
         (define (r . arts) (make-music 'RestEvent 'duration (ly:make-duration 2) 'articulations arts))
         (define (s . arts) (make-music 'SkipEvent 'duration (ly:make-duration 2) 'articulations arts))
         (define (dyn t . props) (apply make-music 'AbsoluteDynamicEvent 'text t props))
         (define (cresc d . props) (apply make-music 'CrescendoEvent 'span-direction d props))
         (define (decresc d . props) (apply make-music 'DecrescendoEvent 'span-direction d props))
         (define (brk) (make-music 'BreakDynamicSpanEvent))
         ;; `\\override`'s music, which replaces an override of the property.
         (define (override grob prop value)
           (make-music 'OverrideProperty 'symbol grob 'grob-property-path (list prop)
                       'grob-value value 'pop-first #t))
         (define (revert grob prop)
           (make-music 'RevertProperty 'symbol grob 'grob-property-path (list prop)))
         (make-music 'ContextSpeccedMusic 'context-type 'Voice 'element
          (seq (make-music 'OverrideProperty 'symbol 'Stem 'grob-property-path '(duration-log)
                           'grob-value
                           (lambda (g)
                             (ly:duration-log
                              (ly:event-property (ly:grob-property (ly:grob-property g 'cause) 'cause)
                                                 'duration))))
               (n (dyn \"p\"))
               (n (cresc -1)) (n) (n (cresc 1))
               (e (dyn \"p\") (cresc -1)) (e) (n) (n (dyn \"f\"))
               (r (dyn \"p\"))
               (s (cresc -1)) (s (cresc 1))
               (n (cresc -1)) (n (dyn \"p\" 'direction 1))
               (n (dyn \"f\" 'direction -1))
               (n (cresc -1 'direction 1)) (n) (n (cresc 1))
               (n (cresc -1) (brk)) (n) (n (cresc 1))
               (n (cresc -1)) (n (brk)) (n (decresc -1)) (n (cresc 1))
               (override 'DynamicTextSpanner 'style 'none)
               (n (cresc -1 'span-type 'text 'span-text \"cresc.\")) (n) (n (cresc 1) (dyn \"p\"))
               (revert 'DynamicTextSpanner 'style)
               (n (cresc -1)) (n (decresc -1)) (n (cresc 1))
               (n (cresc -1)) (n (decresc -1 'direction 1)) (n (cresc 1))
               (n (dyn \"fn\")) (n)
               (make-music 'EventChord 'elements (list (note 0 2) (note 2 2) (dyn \"p\")))
               (n (decresc -1))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &NOTE_HEADS_ENGRAVER,
        &REST_ENGRAVER,
        &STEM_ENGRAVER,
        &RHYTHMIC_COLUMN_ENGRAVER,
        &DYNAMIC_ENGRAVER,
        &DYNAMIC_ALIGN_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(
        show_of(&mut vm, "(kept)"),
        "((warning \"unterminated decrescendo\"))"
    );
    assert_eq!(
        show_of(&mut vm, "(reverse probed)"),
        "((DynamicText -1 (DynamicText 0) (DynamicText 0) ((DynamicText 0)) ((NoteHead 0) (Stem 0))) \
          (Hairpin -1 (NoteColumn 1/4) (NoteColumn 3/4) ((Hairpin 1/4)) \
           ((NoteHead 1/4) (Stem 1/4) (NoteHead 1/2) (Stem 1/2) (NoteHead 3/4) (Stem 3/4))) \
          (Hairpin -1 (DynamicText 1) (DynamicText 3/2) ((Hairpin 1) (DynamicText 1) (DynamicText 3/2)) \
           ((NoteHead 1) (Stem 1) (NoteHead 9/8) (Stem 9/8) (NoteHead 5/4) (Stem 5/4) (NoteHead 3/2) (Stem 3/2))) \
          (DynamicText -1 (DynamicText 7/4) (DynamicText 7/4) ((DynamicText 7/4)) ((Rest 7/4) (Stem 7/4))) \
          (Hairpin -1 (PaperColumn 2) (PaperColumn 9/4) ((Hairpin 2)) ()) \
          (Hairpin -1 (NoteColumn 5/2) (DynamicText 11/4) ((Hairpin 5/2)) ((NoteHead 5/2) (Stem 5/2))) \
          (DynamicText 1 (DynamicText 11/4) (DynamicText 11/4) ((DynamicText 11/4)) ((NoteHead 11/4) (Stem 11/4))) \
          (DynamicText -1 (DynamicText 3) (DynamicText 3) ((DynamicText 3)) ((NoteHead 3) (Stem 3))) \
          (Hairpin 1 (NoteColumn 13/4) (NoteColumn 15/4) ((Hairpin 13/4)) \
           ((NoteHead 13/4) (Stem 13/4) (NoteHead 7/2) (Stem 7/2) (NoteHead 15/4) (Stem 15/4))) \
          (Hairpin -1 (NoteColumn 4) (NoteColumn 9/2) ((Hairpin 4)) ()) \
          (Hairpin -1 (NoteColumn 19/4) (NoteColumn 21/4) ((Hairpin 19/4)) ((NoteHead 19/4) (Stem 19/4))) \
          (Hairpin -1 (NoteColumn 21/4) (NoteColumn 11/2) ((Hairpin 21/4)) \
           ((NoteHead 21/4) (Stem 21/4) (NoteHead 11/2) (Stem 11/2))) \
          (DynamicTextSpanner -1 (NoteColumn 23/4) (DynamicText 25/4) ((DynamicTextSpanner 23/4)) ()) \
          (DynamicText -1 (DynamicText 25/4) (DynamicText 25/4) ((DynamicText 25/4)) ((NoteHead 25/4) (Stem 25/4))) \
          (Hairpin -1 (NoteColumn 13/2) (NoteColumn 7) ((Hairpin 13/2) (Hairpin 27/4)) \
           ((NoteHead 13/2) (Stem 13/2) (NoteHead 27/4) (Stem 27/4) (NoteHead 7) (Stem 7))) \
          (Hairpin -1 (NoteColumn 29/4) (NoteColumn 15/2) ((Hairpin 29/4)) ((NoteHead 29/4) (Stem 29/4))) \
          (Hairpin 1 (NoteColumn 15/2) (NoteColumn 31/4) ((Hairpin 15/2)) \
           ((NoteHead 15/2) (Stem 15/2) (NoteHead 31/4) (Stem 31/4))) \
          (DynamicText -1 (DynamicText 8) (DynamicText 8) ((DynamicText 8) (FootnoteProbe 8)) \
           ((NoteHead 8) (Stem 8))) \
          (DynamicText -1 (DynamicText 17/2) (DynamicText 17/2) ((DynamicText 17/2)) \
           ((NoteHead 17/2) (NoteHead 17/2) (Stem 17/2))) \
          (#f -1 (NoteColumn 35/4) () ((#f 35/4)) ((NoteHead 35/4) (Stem 35/4))))"
    );
}

/// Global with BarLine's, VoltaBracket's (its `edge-height` LilyPond's,
/// its `text` the symbol `proc` for `volta-bracket-interface::calc-text`,
/// as the probe writes a procedure), VoltaBracketSpanner's (its `axes` and
/// `outside-staff-priority` LilyPond's) and the columns' descriptions; a
/// Score with `Paper_column_engraver`, `Repeat_acknowledge_engraver`,
/// `Volta_engraver` and `Volta_probe`, a Staff with `Bar_engraver` and the
/// bar types engraver-init.ly's Score sets, and a Voice.
const VOLTA_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((BarLine (glyph . \"|\") (glyph-right . #f)
                 (meta (name . BarLine) (classes Item)
                       (interfaces grob-interface bar-line-interface)))
        (VoltaBracket (edge-height 2.0 . 2.0) (text . proc)
                      (meta (name . VoltaBracket) (classes Spanner)
                            (interfaces grob-interface font-interface horizontal-bracket-interface
                                        line-interface side-position-interface text-interface
                                        volta-bracket-interface volta-interface)))
        (VoltaBracketSpanner (axes 1) (outside-staff-priority . 600)
                             (meta (name . VoltaBracketSpanner) (classes Spanner)
                                   (interfaces grob-interface axis-group-interface
                                               outside-staff-interface side-position-interface
                                               volta-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (default-child Staff) (consists Paper_column_engraver)
      (consists Repeat_acknowledge_engraver) (consists Volta_engraver) (consists Volta_probe))",
    "((context-name Staff) (translator-type Engraver_group)
      (consists Bar_engraver) (accepts Voice) (default-child Voice)
      (assign measureBarType \"|\") (assign submeasureBarType \"!\")
      (assign startRepeatBarType \".|:\") (assign endRepeatBarType \":|.\")
      (assign doubleRepeatBarType \":..:\") (assign sectionBarType \"||\")
      (assign underlyingRepeatBarType \"||\") (assign segnoBarType \"S\")
      (assign caesuraType ()) (assign beatBase 1/4))",
    "((context-name Voice) (translator-type Engraver_group))",
];

/// `Volta_engraver` (MS3 step 20, `volta-engraver.cc`), layout's class
/// found before the music crate's placeholder, with part 4's
/// `Volta_bracket_interface::add_bar` and the image's
/// `volta-bracket::calc-hook-visibility` (`bar-line.scm`, over LilyPond's
/// `volta-bracket-allow-volta-hook-list`): VoltaBrackets of
/// `repeatCommands`' `(volta label)`, ended by `(volta #f)`, the label
/// their `text`, with LilyPond's warning where none runs; VoltaBrackets of
/// volta span events, their `volta-numbers` the event's, a conflicting
/// start's warning, an unrelated stop passed over, trivial repeats
/// unprinted unless `printTrivialVoltaRepeats`, an empty alternative's
/// killed or discarded by the next start; ended by
/// `voltaBracketMusicalLength`, by `musical-length` at a moment of its own,
/// killed at a `musical-length` of 0; a nested layer's spanner one less in
/// `outside-staff-priority`; a start ending a running bracket with
/// LilyPond's warning. Each bracket takes the bar lines at its timesteps
/// as `bars-left`, or `bars-right` where it ends, and as bounds, else the
/// command columns; its right `edge-height` 0 unless the end bar allows a
/// hook or a D.S. closes it. Each spanner takes the brackets as elements,
/// the bar lines and, where it ends, `stavesFound` as support. As LilyPond
/// 2.26.0 gives the probe's score
/// (`$LILY_CORPUS/device-runs/2026-10-10-ms3-step20/session2-probe/`,
/// `probe.log`).
#[test]
fn volta_brackets_are_engraved_as_lilypond_does() {
    use lily_engine_layout::bar_engraver::BAR_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::volta_engraver::VOLTA_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(VOLTA_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    load_bar_glyph_names(&mut vm);
    load_forms(
        &mut vm,
        "bar-line.scm",
        &["(define-public (volta-bracket::calc-hook-visibility"],
    );
    load_forms(
        &mut vm,
        "output-lib.scm",
        &["(define-public (grob::is-live?"],
    );
    let music = eval(
        &mut vm,
        r#"(for-each
            (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
            `((measureBarType . ,string?) (submeasureBarType . ,string?)
              (startRepeatBarType . ,string?) (endRepeatBarType . ,string?)
              (doubleRepeatBarType . ,string?) (sectionBarType . ,string?)
              (underlyingRepeatBarType . ,string?) (segnoBarType . ,string?)
              (whichBar . ,string?) (caesuraType . ,list?) (beatBase . ,rational?)
              (repeatCommands . ,list?) (printTrivialVoltaRepeats . ,boolean?)
              (voltaBracketMusicalLength . ,ly:moment?) (stavesFound . ,list?)))
           (set-object-property! 'musical-length 'backend-type? ly:moment?)
           (set-object-property! 'VoltaBracket 'is-grob? #t)
           (define-public bar-glyph-alist
             ;; As bar-line.scm's define-bar-line gives them.
             '(("|" . ("|" . #f)) ("||" . ("||" . #f)) (":|." . (":|." . #f))))
           ;; As bar-line.scm's allow-volta-hook gives it.
           (define-public volta-bracket-allow-volta-hook-list '(":|." ":|.S" ":|]" "|." "|.S"))
           (define-public probed '())
           (define (name g) (if (ly:grob? g) (assq-ref (ly:grob-property g 'meta) 'name) g))
           (define (at g)
             (let ((c (if (ly:spanner? g) (ly:spanner-bound g -1) g)))
               (if (ly:grob? c)
                   (list (name g) (ly:moment-main (ly:grob-property (ly:item-get-column c) 'when)))
                   (list (name g) c))))
           (define (bound g d)
             (let ((b (ly:spanner-bound g d)))
               (if (ly:grob? b) (at b) b)))
           (define (arr g sym)
             (let ((a (ly:grob-object g sym)))
               (if (ly:grob-array? a) (map at (ly:grob-array->list a)) a)))
           (define (data g sym)
             (let ((v (ly:grob-property-data g sym)))
               (if (procedure? v) 'proc v)))
           ;; stavesFound the command column at each process-music; at each
           ;; stop, the BarLines acknowledged; at the finalize, each grob of
           ;; volta-interface acknowledged.
           (ly:register-translator
             (lambda (context)
               (let ((made '()) (bars '()))
                 `((acknowledgers
                    (volta-interface . ,(lambda (t g s) (set! made (cons g made))))
                    (bar-line-interface . ,(lambda (t g s) (set! bars (cons g bars)))))
                   (process-music
                    . ,(lambda (t)
                         (ly:context-set-property!
                          context 'stavesFound
                          (list (ly:context-property context 'currentCommandColumn)))))
                   (stop-translation-timestep
                    . ,(lambda (t)
                         (for-each
                          (lambda (g)
                            (set! probed
                                  (cons (list 'bar (at g) (ly:grob-property g 'glyph-left))
                                        probed)))
                          (reverse bars))
                         (set! bars '())))
                   (finalize
                    . ,(lambda (t)
                         (for-each
                          (lambda (g)
                            (let ((cause (ly:grob-property g 'cause)))
                              (set! probed
                                    (cons (list (name g)
                                                (if (ly:stream-event? cause)
                                                    (ly:event-property cause 'volta-numbers)
                                                    cause)
                                                (grob::is-live? g)
                                                (data g 'text)
                                                (data g 'volta-numbers)
                                                (data g 'edge-height)
                                                (data g 'outside-staff-priority)
                                                (bound g -1)
                                                (bound g 1)
                                                (arr g 'bars-left)
                                                (arr g 'bars-right)
                                                (arr g 'elements)
                                                (arr g 'side-support-elements))
                                          probed))))
                          (reverse made)))))))
             'Volta_probe)
           (define (n . arts)
             (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration 2)
                         'articulations arts))
           (define (sc m) (make-music 'ContextSpeccedMusic 'context-type 'Score 'element m))
           (define (st sym value) (sc (make-music 'PropertySet 'symbol sym 'value value)))
           (define (unset sym) (sc (make-music 'PropertyUnset 'symbol sym)))
           (define (bar glyph)
             (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
                         (make-music 'PropertySet 'symbol 'whichBar 'value glyph)))
           (define (nobar)
             (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
                         (make-music 'PropertyUnset 'symbol 'whichBar)))
           (define (repeats . commands) (st 'repeatCommands commands))
           (define (volta d nums count . depth)
             (make-music 'VoltaSpanEvent 'span-direction d 'volta-numbers nums 'repeat-count count
                         'volta-depth (if (pair? depth) (car depth) 0)))
           (define (override prop value)
             (sc (make-music 'OverrideProperty 'symbol 'VoltaBracket 'grob-property-path (list prop)
                             'grob-value value 'pop-first #t)))
           (define (revert prop)
             (sc (make-music 'RevertProperty 'symbol 'VoltaBracket 'grob-property-path (list prop))))
           (define (dal-segno) (make-music 'DalSegnoEvent))
           (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
            (make-music 'ContextSpeccedMusic 'context-type 'Voice 'element
             (seq
              ;; 0: a manual "1."; 1/2: ended at :|. (hook kept), "2."
              ;; started there; 1: ended at | (no hook); 5/4: no bracket to end.
              (repeats '(volta "1.")) (n) (n)
              (bar ":|.") (repeats '(volta #f) '(volta "2.")) (n) (nobar) (n)
              (bar "|") (repeats '(volta #f)) (n) (nobar)
              (repeats '(volta #f)) (n)
              ;; 3/2: a start, a duplicate and a conflicting one; 7/4: an
              ;; unrelated stop alone; 2: an unrelated stop, the stop, a
              ;; start; 9/4: a stop with a D.S.
              (n (volta -1 '(1) 2) (volta -1 '(1) 2) (volta -1 '(3) 2)) (n (volta 1 '(5) 2))
              (n (volta 1 '(2) 2) (volta 1 '(1) 2) (volta -1 '(2) 2))
              (n (volta 1 '(2) 2) (dal-segno))
              ;; 5/2, 11/4: trivial repeats; 3, 13/4: printed.
              (n (volta -1 '(1) 1)) (n (volta 1 '(1) 1))
              (st 'printTrivialVoltaRepeats #t)
              (n (volta -1 '(1) 1)) (n (volta 1 '(1) 1))
              (st 'printTrivialVoltaRepeats #f)
              ;; 7/2: an empty alternative; 15/4: the next; 4: its end.
              (n (volta -1 '(1) 2) (volta 1 '(1) 2))
              (n (volta -1 '(2) 2)) (n (volta 1 '(2) 2))
              ;; 17/4: an empty alternative discarded by the next start.
              (n (volta -1 '(1) 2) (volta 1 '(1) 2) (volta -1 '(2) 2)) (n (volta 1 '(2) 2))
              ;; 19/4: voltaBracketMusicalLength 1/4.
              (st 'voltaBracketMusicalLength (ly:make-moment 1/4))
              (repeats '(volta "a")) (n) (n)
              (unset 'voltaBracketMusicalLength)
              ;; 21/4: musical-length 1/8, which ends it at 43/8; 11/2: length 0.
              (override 'musical-length (ly:make-moment 1/8))
              (repeats '(volta "b")) (n)
              (override 'musical-length (ly:make-moment 0))
              (repeats '(volta "c")) (n)
              (revert 'musical-length)
              ;; 23/4: two layers; 6: the inner ended at :|.; 25/4: the outer.
              (n (volta -1 '(1) 2) (volta -1 '(1) 2 1))
              (bar ":|.") (n (volta 1 '(1) 2 1)) (nobar)
              (n (volta 1 '(1) 2))
              ;; 13/2: a manual "x"; 27/4: "y" ends it prematurely; unterminated.
              (repeats '(volta "x")) (n)
              (repeats '(volta "y")) (n) (n))))"#,
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &BAR_ENGRAVER,
        &VOLTA_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(
        show_of(&mut vm, "(kept)"),
        "((warning \"no VoltaBracket to end\") \
          (warning \"discarding conflicting volta numbers: (3)\") \
          (warning \"already have a VoltaBracket; ending it prematurely\"))"
    );
    assert_eq!(
        show_of(&mut vm, "(reverse probed)"),
        "((bar (BarLine 1/2) \":|.\") \
          (bar (BarLine 1) \"|\") \
          (bar (BarLine 9/4) \"||\") \
          (bar (BarLine 6) \":|.\") \
          (VoltaBracket () #t \"1.\" () (2.0 . 2.0) () (NonMusicalPaperColumn 0) (BarLine 1/2) \
           () ((BarLine 1/2)) () ()) \
          (VoltaBracketSpanner () #t () () () 600 (NonMusicalPaperColumn 0) (BarLine 1) () () \
           ((VoltaBracket 0) (VoltaBracket 1/2)) \
           ((BarLine 1/2) (BarLine 1) (NonMusicalPaperColumn 1))) \
          (VoltaBracket () #t \"2.\" () (2.0 . 0.0) () (BarLine 1/2) (BarLine 1) \
           ((BarLine 1/2)) ((BarLine 1)) () ()) \
          (VoltaBracket (1) #t proc (1) (2.0 . 0.0) () (NonMusicalPaperColumn 3/2) \
           (NonMusicalPaperColumn 2) () () () ()) \
          (VoltaBracketSpanner (1) #t () () () 600 (NonMusicalPaperColumn 3/2) (BarLine 9/4) () () \
           ((VoltaBracket 3/2) (VoltaBracket 2)) ((BarLine 9/4) (NonMusicalPaperColumn 9/4))) \
          (VoltaBracket (2) #t proc (2) (2.0 . 2.0) () (NonMusicalPaperColumn 2) (BarLine 9/4) \
           () ((BarLine 9/4)) () ()) \
          (VoltaBracket (1) #t proc (1) (2.0 . 0.0) () (NonMusicalPaperColumn 3) \
           (NonMusicalPaperColumn 13/4) () () () ()) \
          (VoltaBracketSpanner (1) #t () () () 600 (NonMusicalPaperColumn 3) \
           (NonMusicalPaperColumn 13/4) () () ((VoltaBracket 3)) ((NonMusicalPaperColumn 13/4))) \
          (#f (1) #f () () () () () () () () () ()) \
          (VoltaBracketSpanner (1) #t () () () 600 (NonMusicalPaperColumn 15/4) \
           (NonMusicalPaperColumn 4) () () ((#f ()) (VoltaBracket 15/4)) ((NonMusicalPaperColumn 4))) \
          (VoltaBracket (2) #t proc (2) (2.0 . 0.0) () (NonMusicalPaperColumn 15/4) \
           (NonMusicalPaperColumn 4) () () () ()) \
          (VoltaBracket (2) #t proc (2) (2.0 . 0.0) () (NonMusicalPaperColumn 17/4) \
           (NonMusicalPaperColumn 9/2) () () () ()) \
          (VoltaBracketSpanner (2) #t () () () 600 (NonMusicalPaperColumn 17/4) \
           (NonMusicalPaperColumn 9/2) () () ((VoltaBracket 17/4)) ((NonMusicalPaperColumn 9/2))) \
          (VoltaBracket () #t \"a\" () (2.0 . 0.0) () (NonMusicalPaperColumn 19/4) \
           (NonMusicalPaperColumn 5) () () () ()) \
          (VoltaBracketSpanner () #t () () () 600 (NonMusicalPaperColumn 19/4) \
           (NonMusicalPaperColumn 5) () () ((VoltaBracket 19/4)) ((NonMusicalPaperColumn 5))) \
          (VoltaBracket () #t \"b\" () (2.0 . 0.0) () (NonMusicalPaperColumn 21/4) \
           (NonMusicalPaperColumn 43/8) () () () ()) \
          (VoltaBracketSpanner () #t () () () 600 (NonMusicalPaperColumn 21/4) \
           (NonMusicalPaperColumn 43/8) () () ((VoltaBracket 21/4)) ((NonMusicalPaperColumn 43/8))) \
          (#f () #f () () () () () () () () () ()) \
          (VoltaBracketSpanner () #t () () () 600 (NonMusicalPaperColumn 23/4) \
           (NonMusicalPaperColumn 25/4) () () ((#f ()) (VoltaBracket 23/4)) \
           ((BarLine 6) (NonMusicalPaperColumn 25/4))) \
          (VoltaBracket (1) #t proc (1) (2.0 . 0.0) () (NonMusicalPaperColumn 23/4) \
           (NonMusicalPaperColumn 25/4) ((BarLine 6)) () () ()) \
          (VoltaBracket (1) #t proc (1) (2.0 . 2.0) () (NonMusicalPaperColumn 23/4) (BarLine 6) \
           () ((BarLine 6)) () ()) \
          (VoltaBracketSpanner (1) #t () () () 599 (NonMusicalPaperColumn 23/4) (BarLine 6) () () \
           ((VoltaBracket 23/4)) ((BarLine 6) (NonMusicalPaperColumn 6))) \
          (VoltaBracket () #t \"x\" () (2.0 . 0.0) () (NonMusicalPaperColumn 13/2) \
           (NonMusicalPaperColumn 27/4) () () () ()) \
          (VoltaBracketSpanner () #t () () () 600 (NonMusicalPaperColumn 13/2) \
           (NonMusicalPaperColumn 27/4) () () ((VoltaBracket 13/2) (VoltaBracket 27/4)) ()) \
          (VoltaBracket () #t \"y\" () (2.0 . 2.0) () (NonMusicalPaperColumn 27/4) () () () () ()))"
    );
}

/// Global with NoteHead's, NoteColumn's, OttavaBracket's, TrillSpanner's,
/// PercentRepeat's and the columns' descriptions; a Score with
/// `Paper_column_engraver` and `Bracket_probe`, a Staff with
/// `Ottava_spanner_engraver`, `Staff_probe`, the `ottavationMarkups`
/// (`ottavation-numbers`) and middle C engraver-init.ly's sets, and a
/// Voice with the heads' and the columns' engravers and `Spanner_maker`.
const OTTAVA_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface rhythmic-head-interface)))
        (NoteColumn (axes 0 1)
                    (meta (name . NoteColumn) (classes Item)
                          (interfaces grob-interface axis-group-interface note-column-interface)))
        (OttavaBracket (meta (name . OttavaBracket) (classes Spanner)
                             (interfaces grob-interface font-interface horizontal-bracket-interface
                                         line-interface ottava-bracket-interface
                                         outside-staff-interface side-position-interface
                                         text-interface)))
        (TrillSpanner (meta (name . TrillSpanner) (classes Spanner)
                            (interfaces grob-interface line-interface line-spanner-interface
                                        trill-spanner-interface)))
        (PercentRepeat (meta (name . PercentRepeat) (classes Spanner)
                             (interfaces grob-interface font-interface
                                         multi-measure-rest-interface percent-repeat-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (default-child Staff) (consists Paper_column_engraver) (consists Bracket_probe))",
    "((context-name Staff) (translator-type Engraver_group) (accepts Voice)
      (default-child Voice) (consists Ottava_spanner_engraver) (consists Staff_probe)
      (assign ottavationMarkups ((4 . \"29\") (3 . \"22\") (2 . \"15\") (1 . \"8\")
                                 (-1 . \"8\") (-2 . \"15\") (-3 . \"22\") (-4 . \"29\")))
      (assign middleCClefPosition -6) (assign middleCPosition -6))",
    "((context-name Voice) (translator-type Engraver_group) (consists Note_heads_engraver)
      (consists Rhythmic_column_engraver) (consists Spanner_maker))",
];

/// `Ottava_spanner_engraver` (MS3 step 20, `ottava-engraver.cc`), layout's
/// class found before the music crate's placeholder: `ottavaStartNow`
/// `'()` at each timestep and `#t` at a bracket's start; at each ottava
/// event `middleCOffset` −7 times its number and `middleCPosition` by the
/// image's `ly:set-middle-C!`, from `middleCCuePosition` where it is a
/// number. An OttavaBracket of each number but 0, caused by its event, its
/// `text` `ottavation`, else `ottavationMarkups`' entry, else `""` with
/// LilyPond's warning, an override's kept; its `direction` down for a
/// positive offset, else up, an override's kept. Each takes the note
/// columns as support, the first as its left bound, else the musical
/// column; it ends at the next event, or unterminated, at the last note
/// column but a trilled one, or the command column where a percent repeat
/// came after the last or none came since the previous bracket's end. As
/// LilyPond 2.26.0 gives the probe's score
/// (`$LILY_CORPUS/device-runs/2026-10-10-ms3-step20/session4-probe/`,
/// `probe.log`). T25's own bracket, which needs the core to compile, is
/// compared against its capture natively and in wasm by
/// `tools/compare/test/grobs.test.mjs`.
#[test]
fn ottava_brackets_and_middle_c_are_set_as_lilypond_does() {
    use lily_engine_layout::note_heads_engraver::NOTE_HEADS_ENGRAVER;
    use lily_engine_layout::ottava_engraver::OTTAVA_SPANNER_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::rhythmic_column_engraver::RHYTHMIC_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(OTTAVA_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    load_forms(
        &mut vm,
        "output-lib.scm",
        &["(define-public (grob::is-live?"],
    );
    let music = eval(
        &mut vm,
        r#"(for-each
            (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
            `((ottavationMarkups . ,list?) (middleCClefPosition . ,number?)
              (middleCPosition . ,number?) (ottavation . ,string?)
              (middleCCuePosition . ,number?)))
           (set-object-property! 'OttavaBracket 'is-grob? #t)
           (set-object-property! 'text 'backend-type? string?)
           (set-object-property! 'direction 'backend-type? integer?)
           (define-public probed '())
           (define (name g) (if (ly:grob? g) (assq-ref (ly:grob-property g 'meta) 'name) g))
           (define (at g)
             (let ((c (if (ly:spanner? g) (ly:spanner-bound g -1) g)))
               (if (ly:grob? c)
                   (list (name g) (ly:moment-main (ly:grob-property (ly:item-get-column c) 'when)))
                   (list (name g) c))))
           (define (bound g d)
             (let ((b (ly:spanner-bound g d)))
               (if (ly:grob? b) (at b) b)))
           (define (arr g sym)
             (let ((a (ly:grob-object g sym)))
               (if (ly:grob-array? a) (map at (ly:grob-array->list a)) a)))
           (define (now context) (ly:moment-main (ly:context-current-moment context)))
           ;; At each stop, the moment, ottavaStartNow, middleCOffset and
           ;; middleCPosition.
           (ly:register-translator
             (lambda (context)
               `((stop-translation-timestep
                  . ,(lambda (t)
                       (set! probed
                             (cons (list (now context)
                                         (ly:context-property context 'ottavaStartNow)
                                         (ly:context-property context 'middleCOffset)
                                         (ly:context-property context 'middleCPosition))
                                   probed))))))
             'Staff_probe)
           ;; A TrillSpanner at 1/2 and a PercentRepeat at 5/4, bounded by
           ;; the musical column.
           (ly:register-translator
             (lambda (context)
               (let ((made '()))
                 `((process-music
                    . ,(lambda (t)
                         (let ((type (cond ((equal? (now context) 1/2) 'TrillSpanner)
                                           ((equal? (now context) 5/4) 'PercentRepeat)
                                           (else #f))))
                           (if type
                               (set! made (cons (ly:engraver-make-grob t type '()) made))))))
                   (stop-translation-timestep
                    . ,(lambda (t)
                         (for-each
                          (lambda (g)
                            (let ((col (ly:context-property context 'currentMusicalColumn)))
                              (ly:spanner-set-bound! g -1 col)
                              (ly:spanner-set-bound! g 1 col)))
                          made)
                         (set! made '()))))))
             'Spanner_maker)
           ;; At the finalize, each grob of ottava-bracket-interface.
           (ly:register-translator
             (lambda (context)
               (let ((made '()))
                 `((acknowledgers
                    (ottava-bracket-interface . ,(lambda (t g s) (set! made (cons g made)))))
                   (finalize
                    . ,(lambda (t)
                         (for-each
                          (lambda (g)
                            (let ((cause (ly:grob-property g 'cause)))
                              (set! probed
                                    (cons (list (name g)
                                                (if (ly:stream-event? cause)
                                                    (ly:event-property cause 'ottava-number)
                                                    cause)
                                                (grob::is-live? g)
                                                (ly:grob-property-data g 'text)
                                                (ly:grob-property-data g 'direction)
                                                (bound g -1)
                                                (bound g 1)
                                                (arr g 'side-support-elements))
                                          probed))))
                          (reverse made)))))))
             'Bracket_probe)
           (define (n)
             (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration 2)))
           (define (s) (make-music 'SkipEvent 'duration (ly:make-duration 2)))
           (define (ottava k) (make-music 'OttavaEvent 'ottava-number k))
           (define (staff m) (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element m))
           (define (st sym value) (staff (make-music 'PropertySet 'symbol sym 'value value)))
           (define (unset sym) (staff (make-music 'PropertyUnset 'symbol sym)))
           (define (override prop value)
             (staff (make-music 'OverrideProperty 'symbol 'OttavaBracket
                                'grob-property-path (list prop) 'grob-value value 'pop-first #t)))
           (define (revert prop)
             (staff (make-music 'RevertProperty 'symbol 'OttavaBracket
                                'grob-property-path (list prop))))
           (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
            (make-music 'ContextSpeccedMusic 'context-type 'Voice 'element
             (seq
              ;; 0: 8va over 0, 1/4 and 1/2, the trilled note at 1/2 not its
              ;; end; 3/4: 8vb, down.
              (ottava 1) (n) (n) (n)
              (ottava -1) (n) (n)
              ;; 5/4: a percent repeat at a skip; 3/2: ended at the command
              ;; column.
              (s)
              (ottava 0) (n) (n)
              ;; 2: \set Staff.ottavation, at a skip (the musical column its
              ;; left bound); 9/4: no markup for 5 (warning, ""), the 2's
              ;; end the command column.
              (st 'ottavation "x") (ottava 2) (s)
              (unset 'ottavation) (ottava 5) (n)
              ;; 5/2: middleCCuePosition 3, an override's text; 11/4: an
              ;; override's direction.
              (st 'middleCCuePosition 3) (override 'text "t") (ottava 1) (n)
              (unset 'middleCCuePosition) (revert 'text) (override 'direction -1) (ottava 1) (n)
              (revert 'direction) (n) (n)
              ;; 7/2: 15ma at a skip, with no note column to its end at 15/4
              ;; (the command column); 4: unterminated.
              (ottava 2) (s)
              (ottava 0) (n)
              (ottava 1) (n))))"#,
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &NOTE_HEADS_ENGRAVER,
        &RHYTHMIC_COLUMN_ENGRAVER,
        &OTTAVA_SPANNER_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(
        show_of(&mut vm, "(kept)"),
        "((warning \"Could not find ottavation markup for 5 octaves up.\"))"
    );
    assert_eq!(
        show_of(&mut vm, "(reverse probed)"),
        "((0 #t -7 -13) \
          (1/4 () -7 -13) \
          (1/2 () -7 -13) \
          (3/4 #t 7 1) \
          (1 () 7 1) \
          (5/4 () 7 1) \
          (3/2 () 0 -6) \
          (7/4 () 0 -6) \
          (2 #t -14 -20) \
          (9/4 #t -35 -41) \
          (5/2 #t -7 -4) \
          (11/4 #t -7 -13) \
          (3 () -7 -13) \
          (13/4 () -7 -13) \
          (7/2 #t -14 -20) \
          (15/4 () 0 -6) \
          (4 #t -7 -13) \
          (17/4 () -7 -13) \
          (OttavaBracket 1 #t \"8\" 1 (NoteColumn 0) (NoteColumn 1/4) \
           ((NoteColumn 0) (NoteColumn 1/4) (NoteColumn 1/2))) \
          (OttavaBracket -1 #t \"8\" -1 (NoteColumn 3/4) (NonMusicalPaperColumn 3/2) \
           ((NoteColumn 3/4) (NoteColumn 1))) \
          (OttavaBracket 2 #t \"x\" 1 (PaperColumn 2) (NonMusicalPaperColumn 9/4) ()) \
          (OttavaBracket 5 #t \"\" 1 (NoteColumn 9/4) (NoteColumn 9/4) ((NoteColumn 9/4))) \
          (OttavaBracket 1 #t \"t\" 1 (NoteColumn 5/2) (NoteColumn 5/2) ((NoteColumn 5/2))) \
          (OttavaBracket 1 #t \"8\" -1 (NoteColumn 11/4) (NoteColumn 13/4) \
           ((NoteColumn 11/4) (NoteColumn 3) (NoteColumn 13/4))) \
          (OttavaBracket 2 #t \"15\" 1 (PaperColumn 7/2) (NonMusicalPaperColumn 15/4) ()) \
          (OttavaBracket 1 #t \"8\" 1 (NoteColumn 4) (NoteColumn 4) ((NoteColumn 4))))"
    );
}

/// Alignments, break alignments and hara-kiri groups as `Align_maker`
/// makes them of each note: a VerticalAlignment, two VerticalAxisGroups, a
/// NonMusicalPaperColumn, two NoteHeads, a DynamicText, a
/// DynamicLineSpanner, a BreakAlignment and four BreakAlignGroups, with
/// `define-grobs.scm`'s `axes`.
const ALIGN_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((VerticalAlignment (axes 1)
         (meta (name . VerticalAlignment) (classes Spanner)
               (interfaces grob-interface align-interface axis-group-interface)))
        (VerticalAxisGroup (axes 1)
         (meta (name . VerticalAxisGroup) (classes Spanner)
               (interfaces grob-interface axis-group-interface
                           hara-kiri-group-spanner-interface)))
        (NonMusicalPaperColumn
         (meta (name . NonMusicalPaperColumn) (classes Item)
               (interfaces grob-interface paper-column-interface)))
        (NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface)))
        (DynamicText (meta (name . DynamicText) (classes Item)
                           (interfaces grob-interface dynamic-interface)))
        (DynamicLineSpanner
         (meta (name . DynamicLineSpanner) (classes Spanner)
               (interfaces grob-interface axis-group-interface dynamic-interface)))
        (BreakAlignment (axes 0)
         (meta (name . BreakAlignment) (classes Item)
               (interfaces grob-interface break-alignment-interface align-interface
                           axis-group-interface)))
        (BreakAlignGroup (axes 0)
         (meta (name . BreakAlignGroup) (classes Item)
               (interfaces grob-interface break-align-group-interface
                           axis-group-interface))))))",
    DEFS[1],
    "((context-name Voice) (translator-type Engraver_group) (consists Align_maker))",
];

/// The probe's records (`records.scm` beside it): what the three functions
/// leave on an alignment's elements and on a hara-kiri group, written the
/// same in LilyPond and here.
const ALIGN_RECORDS: &str = "
(define (gname g) (if (ly:grob? g) (assq-ref (ly:grob-property g 'meta) 'name) g))
(define (pname p)
  (cond ((eq? p ly:grob::x-parent-positioning) 'x-parent-positioning)
        ((eq? p ly:grob::y-parent-positioning) 'y-parent-positioning)
        ((procedure? p) 'proc)
        (else p)))
(define (rel me other) (cond ((null? other) '()) ((eq? me other) 'me) (else (gname other))))
(define (aligned me)
  (cons* (gname me) (ly:grob-property-data me 'axes)
         (map (lambda (e)
                (list (gname e) (ly:grob-property-data e 'break-align-symbol)
                      (pname (ly:grob-property-data e 'X-offset))
                      (pname (ly:grob-property-data e 'Y-offset))
                      (rel me (ly:grob-parent e X)) (rel me (ly:grob-parent e Y))
                      (rel me (ly:grob-object e 'axis-group-parent-X))
                      (rel me (ly:grob-object e 'axis-group-parent-Y))))
              (ly:grob-array->list (ly:grob-object me 'elements)))))
(define (interesting me)
  (let ((items (ly:grob-object me 'items-worth-living)))
    (list (gname me) (if (ly:grob-array? items) (map gname (ly:grob-array->list items)) items))))
(define (record g)
  (cond ((memq (gname g) '(VerticalAlignment BreakAlignment)) (aligned g))
        ((eq? (gname g) 'VerticalAxisGroup) (interesting g))
        (else #f)))";

/// Part 3's and part 4's functions that axis groups and alignments take
/// their elements with (MS3 step 21), as `align-interface.cc`,
/// `break-alignment-interface.cc` and `hara-kiri-group-spanner.cc` give
/// them, called as `Vertical_align_engraver`, `Break_align_engraver` and
/// `Axis_group_engraver` call them, on the grobs `Align_maker` makes, which
/// no engraver of layout's has joined yet. The records are LilyPond
/// 2.26.0's of the probe's two staves, the second's `remove-empty` set,
/// at the first timestep (`probe.ly`), and of its BreakAlignment with
/// `axes` `(1 0)` (`probe-b.ly`); an `axes` of `(7 1)` or `(1 7)` is
/// Guile's range error there (`probe-c.ly`, `probe-e.ly`), and none
/// crashes it (`probe-d.ly`), which the engine gives as Guile's `car`
/// would; and `ly:grob::y-parent-positioning` on a staff's group, on a
/// grob with no Y parent and on no grob, and
/// `ly:axis-group-interface::add-element` on `(1 7)` (`probe-y.ly`)
/// (`$LILY_CORPUS/device-runs/2026-10-11-ms3-step21/session1-probe/`).
#[test]
fn alignments_and_hara_kiri_groups_take_their_elements_as_lilypond_does() {
    use lily_engine_layout::{align, break_alignment, hara_kiri_group_spanner};
    let Some((vm, layout)) = vm_with(ALIGN_DEFS) else {
        return;
    };
    let (mut vm, _done) = glided_in(vm, layout, "(seq (note 0 2) (note 1 2))");
    // `X` and `Y` as `lily-library.scm` defines them.
    eval(&mut vm, "(define-public X 0) (define-public Y 1)");
    eval(&mut vm, ALIGN_RECORDS);
    eval(&mut vm, "(define (a n) (list-ref align-made n))");
    let a: Vec<Value> = (0..26)
        .map(|n| eval(&mut vm, &format!("(a {n})")))
        .collect();
    let record = |vm: &mut Vm, g: Value| {
        let f = eval(vm, "record");
        let r = vm
            .apply(f, &[g])
            .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
        show(vm, r)
    };
    let symbols = ["left-edge", "clef", "key-signature", "time-signature"];
    let break_align = |vm: &mut Vm, ba: Value, groups: &[Value]| {
        // `add_to_group` (`break-align-engraver.cc:149–154`): each group's
        // symbol and Y parent set before it is added.
        for (&g, sym) in groups.iter().zip(symbols) {
            let sym = vm.heap_mut().intern(sym);
            lily_engine_layout::grob::set_property(vm.heap_mut(), g, "break-align-symbol", sym);
            set_parent(vm.heap(), g, ba, Y);
            break_alignment::add_element(vm, ba, g).unwrap();
        }
    };
    // The staves' groups have their left bounds' columns as X parents when
    // the alignment takes them; the second staff's keeps its note heads
    // and dynamics alive.
    let [va, vag, vag2, col, head, head2, text, line, ba] = a[..9] else {
        unreachable!()
    };
    set_parent(vm.heap(), vag, col, X);
    set_parent(vm.heap(), vag2, col, X);
    align::add_element(&mut vm, va, vag).unwrap();
    align::add_element(&mut vm, va, vag2).unwrap();
    for n in [head, head2, text, line] {
        hara_kiri_group_spanner::add_interesting_item(&mut vm, vag2, n).unwrap();
    }
    break_align(&mut vm, ba, &a[9..13]);
    let records: Vec<String> = [vag, vag2, va, ba]
        .into_iter()
        .map(|g| record(&mut vm, g))
        .collect();
    assert_eq!(
        records,
        [
            "(VerticalAxisGroup ())",
            "(VerticalAxisGroup (NoteHead NoteHead DynamicText DynamicLineSpanner))",
            "(VerticalAlignment (1) \
             (VerticalAxisGroup () () y-parent-positioning NonMusicalPaperColumn me () me) \
             (VerticalAxisGroup () () y-parent-positioning NonMusicalPaperColumn me () me))",
            "(BreakAlignment (0) \
             (BreakAlignGroup left-edge x-parent-positioning () me me me ()) \
             (BreakAlignGroup clef x-parent-positioning () me me me ()) \
             (BreakAlignGroup key-signature x-parent-positioning () me me me ()) \
             (BreakAlignGroup time-signature x-parent-positioning () me me me ()))",
        ]
    );
    // `ly:grob::y-parent-positioning` reads the Y parent's
    // `positioning-done` as the X one reads the X parent's, and gives 0.0
    // (`probe-y.ly`).
    eval(
        &mut vm,
        "(define done '())
         (define (positioned g) (set! done (append done (list (list 'positioning-done (gname g))))) #t)
         (%grob-set-property-data! (a 0) 'positioning-done positioned)
         (%grob-set-property-data! (a 3) 'positioning-done positioned)",
    );
    check(
        &mut vm,
        &[(
            "(let* ((group (list (gname (a 1)) (ly:grob::y-parent-positioning (a 1))
                                 (ly:grob::x-parent-positioning (a 1))))
                    (orphan (list 'orphan (ly:grob::y-parent-positioning (a 16))))
                    (wrong (catch #t (lambda () (ly:grob::y-parent-positioning 'x))
                                  (lambda args args))))
               (append done (list group orphan wrong)))",
            "((positioning-done VerticalAlignment) (positioning-done NonMusicalPaperColumn) \
             (VerticalAxisGroup 0.0 0.0) (orphan 0.0) \
             (wrong-type-arg \"ly:y-parent-positioning\" \
             \"Wrong type argument in position ~A (expecting ~A): ~S\" (1 \"Grob\" x) (x)))",
        )],
    );
    // `items-worth-living` is unordered.
    let sym = vm.heap_mut().intern("items-worth-living");
    let arr = get_grob_array(&mut vm, vag2, sym).unwrap();
    assert!(!vm.heap().foreign::<GrobArray>(arr).unwrap().ordered);
    // A break alignment on `(1 0)`: its groups' Y-offset set, both
    // parents and both axis group parents its.
    let [va, vag, vag2, ba] = [a[13], a[14], a[15], a[21]];
    eval(&mut vm, "(%grob-set-property-data! (a 21) 'axes '(1 0))");
    break_align(&mut vm, ba, &a[22..26]);
    assert_eq!(
        record(&mut vm, ba),
        "(BreakAlignment (1 0) \
         (BreakAlignGroup left-edge () y-parent-positioning me me me me) \
         (BreakAlignGroup clef () y-parent-positioning me me me me) \
         (BreakAlignGroup key-signature () y-parent-positioning me me me me) \
         (BreakAlignGroup time-signature () y-parent-positioning me me me me))"
    );
    // An axis that is no axis: the first is taken as X, and Guile's range
    // error is thrown where the axis group reaches it, the axes before it
    // taken and the element not added.
    eval(&mut vm, "(%grob-set-property-data! (a 13) 'axes '(7 1))");
    let e = align::add_element(&mut vm, va, vag).unwrap_err();
    assert_eq!(e.text(vm.heap()), "Value out of range 0 to< 1: 7");
    eval(&mut vm, "(%grob-set-property-data! (a 13) 'axes '(1 7))");
    let e = align::add_element(&mut vm, va, vag2).unwrap_err();
    assert_eq!(e.text(vm.heap()), "Value out of range 0 to< 1: 7");
    check(
        &mut vm,
        &[
            (
                "(map (lambda (g) (list (pname (ly:grob-property-data g 'X-offset)) (pname (ly:grob-property-data g 'Y-offset)) (rel (a 13) (ly:grob-parent g X)) (rel (a 13) (ly:grob-parent g Y)) (rel (a 13) (ly:grob-object g 'axis-group-parent-Y)))) (list (a 14) (a 15)))",
                "((x-parent-positioning () () () ()) (() y-parent-positioning () me me))",
            ),
            ("(ly:grob-object (a 13) 'elements)", "()"),
        ],
    );
    // The Scheme face converts the axes as it reaches them too.
    check(
        &mut vm,
        &[(
            "(begin
               (%grob-set-property-data! (a 14) 'axes '(1 7))
               (list (catch #t (lambda () (ly:axis-group-interface::add-element (a 14) (a 18)))
                            (lambda args args))
                     (list (eq? (ly:grob-parent (a 18) Y) (a 14))
                           (eq? (ly:grob-object (a 18) 'axis-group-parent-Y) (a 14))
                           (ly:grob-object (a 14) 'elements))))",
            "((out-of-range #f \"Value out of range ~S to< ~S: ~S\" (0 1 7) (7)) (#t #t ()))",
        )],
    );
    // No axes: Guile's error from `car`.
    eval(&mut vm, "(%grob-set-property-data! (a 13) 'axes '())");
    let e = align::add_element(&mut vm, va, vag).unwrap_err();
    assert_eq!(
        e.text(vm.heap()),
        "In procedure car: Wrong type argument (expecting pair): ()"
    );
}

/// Global with StaffSymbol's, NoteHead's, NoteColumn's, DynamicText's,
/// VerticalAxisGroup's and the columns' descriptions; a Score with
/// `Paper_column_engraver`, `Score_probe`, `Context_probe` and
/// `keepAliveInterfaces`; a Staff with `Staff_symbol_engraver`,
/// `Axis_group_engraver` and `Context_probe`; a Voice with the heads' and
/// the columns' engravers, `Dynamic_maker` and `Context_probe`; an Inner
/// whose `hasAxisGroup` is `#f` and a Lone whose `hasAxisGroup` is `#t`,
/// each with `Note_heads_engraver`, `Axis_group_engraver` and
/// `Context_probe`.
const AXIS_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((StaffSymbol (meta (name . StaffSymbol) (classes Spanner)
                           (interfaces grob-interface staff-symbol-interface)))
        (NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface rhythmic-head-interface)))
        (NoteColumn (axes 0 1)
                    (meta (name . NoteColumn) (classes Item)
                          (interfaces grob-interface axis-group-interface note-column-interface)))
        (DynamicText (meta (name . DynamicText) (classes Item)
                           (interfaces grob-interface dynamic-interface dynamic-text-interface)))
        (VerticalAxisGroup (axes 1)
                           (meta (name . VerticalAxisGroup) (classes Spanner)
                                 (interfaces grob-interface axis-group-interface
                                             hara-kiri-group-spanner-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff) (accepts Lone)
      (consists Paper_column_engraver) (consists Score_probe) (consists Context_probe)
      (assign keepAliveInterfaces (dynamic-interface note-head-interface)))",
    "((context-name Staff) (translator-type Engraver_group) (accepts Voice) (accepts Inner)
      (default-child Voice) (consists Staff_symbol_engraver) (consists Axis_group_engraver)
      (consists Context_probe) (assign middleCPosition -6))",
    "((context-name Voice) (translator-type Engraver_group) (consists Note_heads_engraver)
      (consists Rhythmic_column_engraver) (consists Dynamic_maker) (consists Context_probe))",
    "((context-name Inner) (translator-type Engraver_group) (consists Note_heads_engraver)
      (consists Axis_group_engraver) (consists Context_probe) (assign hasAxisGroup #f))",
    "((context-name Lone) (translator-type Engraver_group) (consists Note_heads_engraver)
      (consists Axis_group_engraver) (consists Context_probe) (assign hasAxisGroup #t)
      (assign middleCPosition -6))",
];

/// The probes of [`AXIS_DEFS`] and the music's makers, written as the
/// probe's `.ly` files write them, each record given to `probe`.
const AXIS_PROBES: &str = "
    (for-each (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
              `((keepAliveInterfaces . ,list?) (hasAxisGroup . ,boolean?)
                (middleCPosition . ,number?)))
    (set-object-property! 'VerticalAxisGroup 'is-grob? #t)
    (set-object-property! 'remove-empty 'backend-type? boolean?)
    (define-public probed '())
    (define-public axis-groups '())
    (define (probe x) (set! probed (cons x probed)))
    (define (name g) (if (ly:grob? g) (assq-ref (ly:grob-property g 'meta) 'name) g))
    (define (at g)
      (let ((c (if (ly:spanner? g) (ly:spanner-bound g -1) g)))
        (if (ly:grob? c)
            (list (name g) (ly:moment-main (ly:grob-property (ly:item-get-column c) 'when)))
            (list (name g) c))))
    (define (bound g d)
      (let ((b (ly:spanner-bound g d)))
        (if (ly:grob? b) (at b) b)))
    (define (arr g sym)
      (let ((a (ly:grob-object g sym)))
        (if (ly:grob-array? a) (map at (ly:grob-array->list a)) a)))
    (define (now context) (ly:moment-main (ly:context-current-moment context)))
    ;; At the initialize, the context's name and hasAxisGroup; at the first
    ;; stop, the context's name, the moment, hasAxisGroup and the context
    ;; that defines it.
    (ly:register-translator
      (lambda (context)
        (let ((done #f))
          `((initialize
             . ,(lambda (t)
                  (probe (list 'initialize (ly:context-name context)
                               (ly:context-property context 'hasAxisGroup)))))
            (stop-translation-timestep
             . ,(lambda (t)
                  (if (not done)
                      (let ((where (ly:context-property-where-defined context 'hasAxisGroup)))
                        (set! done #t)
                        (probe (list (ly:context-name context) (now context)
                                     (ly:context-property context 'hasAxisGroup)
                                     (if (ly:context? where) (ly:context-name where) where))))))))))
      'Context_probe)
    ;; A DynamicText at 1/4.
    (ly:register-translator
      (lambda (context)
        `((process-music
           . ,(lambda (t)
                (if (equal? (now context) 1/4)
                    (ly:engraver-make-grob t 'DynamicText '()))))))
      'Dynamic_maker)
    ;; Each VerticalAxisGroup acknowledged, with the moment; at the
    ;; finalize, each VerticalAxisGroup: its bounds, its elements
    ;; with whether it is their Y parent and axis-group-parent-Y, its
    ;; items-worth-living, its Y parent and its remove-empty; then each
    ;; NoteHead and its axis-group-parent-Y.
    (ly:register-translator
      (lambda (context)
        (let ((groups '()) (heads '()))
          `((acknowledgers
             (hara-kiri-group-spanner-interface
              . ,(lambda (t g s)
                   (probe (list 'acknowledged (name g) (now context)))
                   (set! groups (cons g groups))))
             (note-head-interface . ,(lambda (t g s) (set! heads (cons g heads)))))
            (finalize
             . ,(lambda (t)
                  (set! axis-groups (reverse groups))
                  (for-each
                   (lambda (g)
                     (probe
                      (list (name g) (bound g -1) (bound g 1)
                            (map (lambda (e)
                                   (append (at e)
                                           (list (eq? (ly:grob-parent e 1) g)
                                                 (eq? (ly:grob-object e 'axis-group-parent-Y) g))))
                                 (ly:grob-array->list (ly:grob-object g 'elements)))
                            (arr g 'items-worth-living)
                            (let ((p (ly:grob-parent g 1))) (if (ly:grob? p) (at p) p))
                            (ly:grob-property-data g 'remove-empty))))
                   (reverse groups))
                  (for-each
                   (lambda (h)
                     (probe (list (at h) (name (ly:grob-object h 'axis-group-parent-Y)))))
                   (reverse heads)))))))
      'Score_probe)
    (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
    (define (n) (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration 2)))
    (define (s) (make-music 'SkipEvent 'duration (ly:make-duration 2)))
    (define (in type m) (make-music 'ContextSpeccedMusic 'context-type type 'element m))
    (define (new type m)
      (make-music 'ContextSpeccedMusic 'context-type type 'create-new #t 'element m))
    (define (remove-empty)
      (in 'Staff (make-music 'OverrideProperty 'symbol 'VerticalAxisGroup
                             'grob-property-path '(remove-empty) 'grob-value #t 'pop-first #t)))";

/// `Axis_group_engraver` (MS3 step 21, `axis-group-engraver.cc`), layout's
/// class found before the music crate's placeholder, last in its group,
/// so its context's other translators see `hasAxisGroup` unset at their
/// `initialize`: the context's `hasAxisGroup` `#t` from its first, so that
/// an engraver of a context below it, or of one where it is already `#t`,
/// makes no group; a
/// VerticalAxisGroup of each other, with no cause, from the first command
/// column to the last; its elements the grobs of the context and below it
/// that have no `axis-group-parent-Y` once acknowledged (a note column's
/// heads have their column's, a group below its own), unordered; with
/// `remove-empty` set, those of `keepAliveInterfaces` its
/// `items-worth-living`. As LilyPond 2.26.0 gives the probe's score
/// (`$LILY_CORPUS/device-runs/2026-10-11-ms3-step21/session2-probe/`,
/// `probe.log`).
#[test]
fn axis_groups_take_the_grobs_of_their_contexts_as_lilypond_does() {
    use lily_engine_layout::axis_group_engraver::AXIS_GROUP_ENGRAVER;
    use lily_engine_layout::note_heads_engraver::NOTE_HEADS_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::rhythmic_column_engraver::RHYTHMIC_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::staff_symbol_engraver::STAFF_SYMBOL_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(AXIS_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, AXIS_PROBES);
    let music = eval(
        &mut vm,
        "(new 'Score (par
          ;; A staff of four notes, a DynamicText at 1/4.
          (new 'Staff (in 'Voice (seq (n) (n) (n) (n))))
          ;; remove-empty: its note heads and the DynamicText kept alive.
          (new 'Staff (seq (remove-empty) (in 'Voice (seq (n) (n) (s) (n)))))
          ;; A staff from 1/2, with an Inner whose own group is the staff's
          ;; element.
          (seq (s) (s) (new 'Staff (par (in 'Voice (seq (n) (n))) (new 'Inner (seq (n) (s) (n))))))
          ;; A Lone whose hasAxisGroup is #t: no group.
          (new 'Lone (seq (n) (n)))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &STAFF_SYMBOL_ENGRAVER,
        &NOTE_HEADS_ENGRAVER,
        &RHYTHMIC_COLUMN_ENGRAVER,
        &AXIS_GROUP_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(show_of(&mut vm, "(kept)"), "()");
    assert_eq!(
        show_of(&mut vm, "(reverse probed)"),
        "((initialize Score ()) \
          (initialize Staff ()) \
          (initialize Voice #t) \
          (initialize Staff ()) \
          (initialize Lone #t) \
          (initialize Voice #t) \
          (acknowledged VerticalAxisGroup 0) \
          (acknowledged VerticalAxisGroup 0) \
          (Voice 0 #t Staff) \
          (Staff 0 #t Staff) \
          (Voice 0 #t Staff) \
          (Staff 0 #t Staff) \
          (Lone 0 #t Lone) \
          (Score 0 () ()) \
          (initialize Staff ()) \
          (initialize Voice #t) \
          (initialize Inner #f) \
          (acknowledged VerticalAxisGroup 1/2) \
          (acknowledged VerticalAxisGroup 1/2) \
          (Voice 1/2 #t Staff) \
          (Inner 1/2 #t Inner) \
          (Staff 1/2 #t Staff) \
          (VerticalAxisGroup (NonMusicalPaperColumn 0) (NonMusicalPaperColumn 1) \
           ((StaffSymbol 0 #t #t) (NoteColumn 0 #t #t) (DynamicText 1/4 #t #t) \
            (NoteColumn 1/4 #t #t) (NoteColumn 1/2 #t #t) (NoteColumn 3/4 #t #t)) \
           () (System 0) ()) \
          (VerticalAxisGroup (NonMusicalPaperColumn 0) (NonMusicalPaperColumn 1) \
           ((StaffSymbol 0 #t #t) (NoteColumn 0 #t #t) (DynamicText 1/4 #t #t) \
            (NoteColumn 1/4 #t #t) (NoteColumn 3/4 #t #t)) \
           ((NoteHead 0) (NoteHead 1/4) (DynamicText 1/4) (NoteHead 3/4)) (System 0) #t) \
          (VerticalAxisGroup (NonMusicalPaperColumn 1/2) (NonMusicalPaperColumn 5/4) \
           ((NoteHead 1/2 #t #t) (NoteHead 1 #t #t)) \
           () (VerticalAxisGroup 1/2) ()) \
          (VerticalAxisGroup (NonMusicalPaperColumn 1/2) (NonMusicalPaperColumn 5/4) \
           ((VerticalAxisGroup 1/2 #t #t) (StaffSymbol 1/2 #t #t) (NoteColumn 1/2 #t #t) \
            (NoteColumn 3/4 #t #t)) \
           () (System 0) ()) \
          ((NoteHead 0) NoteColumn) \
          ((NoteHead 0) NoteColumn) \
          ((NoteHead 0) System) \
          ((NoteHead 1/4) NoteColumn) \
          ((NoteHead 1/4) NoteColumn) \
          ((NoteHead 1/4) System) \
          ((NoteHead 1/2) NoteColumn) \
          ((NoteHead 1/2) NoteColumn) \
          ((NoteHead 1/2) VerticalAxisGroup) \
          ((NoteHead 3/4) NoteColumn) \
          ((NoteHead 3/4) NoteColumn) \
          ((NoteHead 3/4) NoteColumn) \
          ((NoteHead 1) VerticalAxisGroup))"
    );
    // The groups' elements are unordered.
    let groups = eval(&mut vm, "axis-groups");
    let elements = vm.heap_mut().intern("elements");
    let mut p = groups;
    while let (Some(g), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
        p = rest;
        let arr = get_grob_array(&mut vm, g, elements).unwrap();
        assert!(!vm.heap().foreign::<GrobArray>(arr).unwrap().ordered);
    }
    assert_eq!(show_of(&mut vm, "(length axis-groups)"), "4");
}

/// [`AXIS_DEFS`] with a Staff whose `Parent_maker` comes before
/// `Axis_group_engraver`, and a Plain that the Score accepts too, with
/// `Axis_group_engraver` and `Context_probe`.
const AXIS_PARENT_DEFS: &[&str] = &[
    AXIS_DEFS[0],
    "((context-name Score) (translator-type Score_engraver) (accepts Staff) (accepts Lone)
      (accepts Plain) (consists Paper_column_engraver) (consists Score_probe)
      (consists Context_probe) (assign keepAliveInterfaces (dynamic-interface note-head-interface)))",
    "((context-name Staff) (translator-type Engraver_group) (accepts Voice)
      (default-child Voice) (consists Staff_symbol_engraver) (consists Parent_maker)
      (consists Axis_group_engraver) (consists Context_probe) (assign middleCPosition -6))",
    AXIS_DEFS[3],
    "((context-name Plain) (translator-type Engraver_group) (accepts Voice)
      (default-child Voice) (consists Axis_group_engraver) (consists Context_probe)
      (assign middleCPosition -6))",
];

/// `Axis_group_engraver`'s group whose Y parent is one of the grobs it
/// acknowledges, as `Parent_maker` makes a DynamicText of each group and
/// makes it the group's Y parent: LilyPond's three warnings and the group's
/// suicide, and another group at each timestep, which dies so; the Score
/// acknowledges none of them, as LilyPond's skips a dead grob, though it
/// has acknowledged a Plain's group, alive, first. As LilyPond
/// 2.26.0 gives the probe's score, its records and warnings in one order
/// (`$LILY_CORPUS/device-runs/2026-10-11-ms3-step21/session2-probe/`,
/// `probe-two.log`).
#[test]
fn an_axis_group_whose_parent_it_takes_is_removed_as_lilypond_does() {
    use lily_engine_layout::axis_group_engraver::AXIS_GROUP_ENGRAVER;
    use lily_engine_layout::note_heads_engraver::NOTE_HEADS_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::rhythmic_column_engraver::RHYTHMIC_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::staff_symbol_engraver::STAFF_SYMBOL_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(AXIS_PARENT_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, AXIS_PROBES);
    let music = eval(
        &mut vm,
        "(define (probe x) (ly:warning \"~S\" x))
         (ly:register-translator
           (lambda (context)
             `((acknowledgers
                (hara-kiri-group-spanner-interface
                 . ,(lambda (t g s)
                      (probe (list 'parent (now context)))
                      (ly:grob-set-parent! g 1 (ly:engraver-make-grob t 'DynamicText '())))))))
           'Parent_maker)
         (new 'Score
              (par (new 'Plain (in 'Voice (seq (n) (n)))) (new 'Staff (in 'Voice (seq (n) (n))))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &STAFF_SYMBOL_ENGRAVER,
        &NOTE_HEADS_ENGRAVER,
        &RHYTHMIC_COLUMN_ENGRAVER,
        &AXIS_GROUP_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    let removed = "(warning \"Axis_group_engraver: vertical group already has a parent\") \
                   (warning \"are there two Axis_group_engravers?\") \
                   (warning \"removing this vertical group\")";
    assert_eq!(
        show_of(&mut vm, "(kept)"),
        format!(
            "((warning \"(initialize Score ())\") \
              (warning \"(initialize Plain ())\") \
              (warning \"(initialize Voice #t)\") \
              (warning \"(initialize Staff ())\") \
              (warning \"(initialize Voice #t)\") \
              (warning \"(parent 0)\") {removed} \
              (warning \"(acknowledged VerticalAxisGroup 0)\") \
              (warning \"(Voice 0 #t Plain)\") \
              (warning \"(Plain 0 #t Plain)\") \
              (warning \"(Voice 0 #t Staff)\") \
              (warning \"(Staff 0 #t Staff)\") \
              (warning \"(Score 0 () ())\") \
              (warning \"(parent 1/4)\") {removed} \
              (warning \"(parent 1/2)\") {removed} \
              (warning \"(VerticalAxisGroup (NonMusicalPaperColumn 0) (NonMusicalPaperColumn 1/2) \
               ((NoteColumn 0 #t #t) (DynamicText 1/4 #t #t) (NoteColumn 1/4 #t #t)) () (System 0) ())\") \
              (warning \"((NoteHead 0) NoteColumn)\") \
              (warning \"((NoteHead 0) NoteColumn)\") \
              (warning \"((NoteHead 1/4) NoteColumn)\") \
              (warning \"((NoteHead 1/4) NoteColumn)\"))"
        )
    );
}

/// Global with TextScript's, VerticalAxisGroup's, VerticalAlignment's,
/// StaffGrouper's and the columns' descriptions; a Score with
/// `Paper_column_engraver`, `Outside_maker`, `Vertical_align_engraver` and
/// `Align_probe`,
/// `topLevelAlignment` `#t`; a Staff with `Outside_maker` and
/// `Axis_group_engraver`; an Above, a Below and a Lost with
/// `Axis_group_engraver` and their `alignAboveContext` or
/// `alignBelowContext`; a Group with `Outside_maker` and
/// `Vertical_align_engraver`, `topLevelAlignment` `#f`; a Grouped with
/// `Outside_maker`, `Vertical_align_engraver` and `Axis_group_engraver`.
const VALIGN_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((TextScript (meta (name . TextScript) (classes Item)
                          (interfaces grob-interface outside-staff-interface text-script-interface)))
        (VerticalAxisGroup (axes 1)
                           (meta (name . VerticalAxisGroup) (classes Spanner)
                                 (interfaces grob-interface axis-group-interface
                                             hara-kiri-group-spanner-interface)))
        (VerticalAlignment (axes 1)
                           (meta (name . VerticalAlignment) (classes Spanner)
                                 (interfaces grob-interface align-interface axis-group-interface)))
        (StaffGrouper (meta (name . StaffGrouper) (classes Spanner)
                            (interfaces grob-interface staff-grouper-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff) (accepts Above)
      (accepts Below) (accepts Lost) (accepts Group) (accepts Grouped)
      (consists Paper_column_engraver) (consists Outside_maker) (consists Vertical_align_engraver)
      (consists Align_probe) (assign topLevelAlignment #t))",
    "((context-name Staff) (translator-type Engraver_group) (consists Outside_maker)
      (consists Axis_group_engraver))",
    "((context-name Above) (translator-type Engraver_group) (consists Axis_group_engraver)
      (assign alignAboveContext \"two\"))",
    "((context-name Below) (translator-type Engraver_group) (consists Axis_group_engraver)
      (assign alignBelowContext \"one\"))",
    "((context-name Lost) (translator-type Engraver_group) (consists Axis_group_engraver)
      (assign alignAboveContext \"none\"))",
    "((context-name Group) (translator-type Engraver_group) (accepts Staff) (accepts Group)
      (consists Outside_maker) (consists Vertical_align_engraver) (assign topLevelAlignment #f))",
    "((context-name Grouped) (translator-type Engraver_group) (consists Outside_maker)
      (consists Vertical_align_engraver) (consists Axis_group_engraver))",
];

/// The probes of [`VALIGN_DEFS`] and the music's makers, written as the
/// probe's `.ly` file writes them, each record a warning.
const VALIGN_PROBES: &str = "
    (for-each (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
              `((topLevelAlignment . ,boolean?) (hasAxisGroup . ,boolean?)
                (alignAboveContext . ,string?) (alignBelowContext . ,string?)))
    (set-object-property! 'VerticalAxisGroup 'is-grob? #t)
    (set-object-property! 'staff-affinity 'backend-type? number?)
    (define labels '())
    (define (gname g) (if (ly:grob? g) (assq-ref (ly:grob-property g 'meta) 'name) g))
    (define (label g) (or (assq-ref labels g) (gname g)))
    (define (pname p)
      (cond ((eq? p ly:grob::y-parent-positioning) 'y-parent-positioning)
            ((procedure? p) 'proc)
            (else p)))
    (define (when-of b)
      (if (ly:grob? b) (ly:moment-main (ly:grob-property (ly:item-get-column b) 'when)) b))
    (define (now context) (ly:moment-main (ly:context-current-moment context)))
    (define (probe x) (ly:warning \"~S\" x))
    ;; A TextScript at 0.
    (ly:register-translator
      (lambda (context)
        `((process-music
           . ,(lambda (t)
                (if (equal? (now context) 0)
                    (ly:engraver-make-grob t 'TextScript '()))))))
      'Outside_maker)
    ;; Each axis group, alignment, staff grouper and outside-staff grob
    ;; acknowledged, labelled by its engraver's context; at the finalize,
    ;; each alignment and staff grouper: its bounds, then, a record each,
    ;; each element's Y parent, Y-offset, axis-group-parent-Y,
    ;; staff-affinity and staff-grouper; then each outside-staff grob's Y
    ;; parent.
    (ly:register-translator
      (lambda (context)
        (let ((aligns '()) (outside '()))
          (define (labelled g source)
            (let* ((c (ly:translator-context source))
                   (l (list (gname g) (ly:context-name c) (ly:context-id c))))
              (set! labels (acons g l labels))
              (probe (list 'acknowledged l (now context)))))
          `((acknowledgers
             (hara-kiri-group-spanner-interface . ,(lambda (t g s) (labelled g s)))
             (align-interface
              . ,(lambda (t g s) (labelled g s) (set! aligns (cons g aligns))))
             (staff-grouper-interface
              . ,(lambda (t g s) (labelled g s) (set! aligns (cons g aligns))))
             (outside-staff-interface
              . ,(lambda (t g s) (labelled g s) (set! outside (cons g outside)))))
            (finalize
             . ,(lambda (t)
                  (for-each
                   (lambda (a)
                     (probe (list (label a) (when-of (ly:spanner-bound a -1))
                                  (when-of (ly:spanner-bound a 1))))
                     (for-each
                      (lambda (e)
                        (probe (list 'element (label e) (label (ly:grob-parent e 1))
                                     (pname (ly:grob-property-data e 'Y-offset))
                                     (label (ly:grob-object e 'axis-group-parent-Y))
                                     (ly:grob-property-data e 'staff-affinity)
                                     (label (ly:grob-object e 'staff-grouper)))))
                      (ly:grob-array->list (ly:grob-object a 'elements))))
                   (reverse aligns))
                  (for-each
                   (lambda (g) (probe (list (label g) (label (ly:grob-parent g 1)))))
                   (reverse outside)))))))
      'Align_probe)
    (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
    (define (s) (make-music 'SkipEvent 'duration (ly:make-duration 2)))
    (define (new type m)
      (make-music 'ContextSpeccedMusic 'context-type type 'create-new #t 'element m))
    (define (new-id type id m)
      (make-music 'ContextSpeccedMusic 'context-type type 'context-id id 'create-new #t
                  'element m))
    (define (affinity type)
      (make-music 'ContextSpeccedMusic 'context-type type 'element
                  (make-music 'OverrideProperty 'symbol 'VerticalAxisGroup
                              'grob-property-path '(staff-affinity) 'grob-value 1
                              'pop-first #t)))";

/// `Vertical_align_engraver` (MS3 step 21, `vertical-align-engraver.cc`),
/// layout's class found before the music crate's placeholder: where
/// `topLevelAlignment` is `#t`, a VerticalAlignment with no cause from the
/// first command column to the last, each axis group of the score one of
/// its elements through `Align_interface::add_element` (its Y parent and
/// `axis-group-parent-Y` the alignment, its `Y-offset`
/// `ly:grob::y-parent-positioning`), in the order they are acknowledged
/// but for those whose context's `alignAboveContext` or
/// `alignBelowContext` names the id of an earlier group's context: put
/// before or after the last group of that id, with a `staff-affinity`
/// that is a number made `DOWN` or `UP`; one that names no such id stays
/// last, with LilyPond's warning. The Score's outside-staff grob is not
/// claimed. Elsewhere a StaffGrouper, the axis groups below it its
/// elements, its own context's outside-staff grobs its children on Y, and
/// itself the `staff-grouper` of each group that has none yet; and where
/// `hasAxisGroup` is `#t`, LilyPond's warning and no grob, and its
/// programming error for an outside-staff grob. As LilyPond 2.26.0 gives
/// the probe's score, records and warnings in one order
/// (`$LILY_CORPUS/device-runs/2026-10-11-ms3-step21/session3-probe/`,
/// `probe.log`, before its "Preprocessing").
#[test]
fn the_vertical_alignment_takes_the_axis_groups_as_lilypond_does() {
    use lily_engine_layout::axis_group_engraver::AXIS_GROUP_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::vertical_align_engraver::VERTICAL_ALIGN_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(VALIGN_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, VALIGN_PROBES);
    let music = eval(
        &mut vm,
        "(new 'Score (par
          (new-id 'Staff \"one\" (seq (s) (s) (s)))
          (new-id 'Staff \"two\" (seq (s) (s) (s)))
          ;; At 1/4: a second \"one\"; above \"two\", its staff-affinity
          ;; UP made DOWN; above \"none\", which no context is. At 1/2:
          ;; below the last \"one\", with no staff-affinity.
          (seq (s) (new-id 'Staff \"one\" (seq (s) (s))))
          (seq (s) (new 'Above (seq (affinity 'Above) (s) (s))))
          (seq (s) (new 'Lost (seq (s) (s))))
          (seq (s) (s) (new 'Below (s)))
          ;; A StaffGrouper with a staff and a Group in it, each with an
          ;; outside-staff grob; the inner staff's staff-grouper is the
          ;; inner Group's, which acknowledges its group first.
          (new-id 'Group \"outer\" (par (seq (s) (s) (s)) (new 'Staff (seq (s) (s) (s)))
                           (new-id 'Group \"inner\" (new 'Staff (seq (s) (s) (s))))))
          ;; Vertical_align_engraver where hasAxisGroup is #t.
          (new 'Grouped (seq (s) (s) (s)))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &AXIS_GROUP_ENGRAVER,
        &VERTICAL_ALIGN_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    let element = |label: &str, affinity: &str, grouper: &str| {
        format!(
            "(element {label} (VerticalAlignment Score \"\") y-parent-positioning \
             (VerticalAlignment Score \"\") {affinity} {grouper})"
        )
    };
    let (inner, outer) = (
        "(StaffGrouper Group \"inner\")",
        "(StaffGrouper Group \"outer\")",
    );
    let (inner_staff, outer_staff) = (
        element("(VerticalAxisGroup Staff \"\")", "()", inner),
        element("(VerticalAxisGroup Staff \"\")", "()", outer),
    );
    let made = |label: &str| format!("(acknowledged {label} 0)");
    let warn = |r: &str| format!("(warning \"{}\")", r.replace('"', "\\\""));
    let mut expected: Vec<String> = vec![
        "(warning \"Ignoring Vertical_align_engraver in VerticalAxisGroup\")".to_string(),
        "(programming-error \"cannot claim outside-staff grob before creating staff grouper\")"
            .to_string(),
    ];
    let records = [
        made("(TextScript Staff \"one\")"),
        made("(VerticalAxisGroup Staff \"one\")"),
        made("(TextScript Staff \"two\")"),
        made("(VerticalAxisGroup Staff \"two\")"),
        made("(TextScript Staff \"\")"),
        made("(VerticalAxisGroup Staff \"\")"),
        made("(TextScript Staff \"\")"),
        made("(VerticalAxisGroup Staff \"\")"),
        made("(TextScript Group \"inner\")"),
        made(inner),
        made("(TextScript Group \"outer\")"),
        made(outer),
        made("(TextScript Grouped \"\")"),
        made("(VerticalAxisGroup Grouped \"\")"),
        made("(TextScript Score \"\")"),
        made("(VerticalAlignment Score \"\")"),
        "(acknowledged (VerticalAxisGroup Staff \"one\") 1/4)".to_string(),
        "(acknowledged (VerticalAxisGroup Above \"\") 1/4)".to_string(),
    ];
    expected.extend(records.iter().map(|r| warn(r)));
    expected.push("(warning \"alignAboveContext not found: none\")".to_string());
    let records = [
        "(acknowledged (VerticalAxisGroup Lost \"\") 1/4)".to_string(),
        "(acknowledged (VerticalAxisGroup Below \"\") 1/2)".to_string(),
        format!("({inner} 0 3/4)"),
        inner_staff.clone(),
        format!("({outer} 0 3/4)"),
        outer_staff.clone(),
        inner_staff.clone(),
        "((VerticalAlignment Score \"\") 0 3/4)".to_string(),
        element("(VerticalAxisGroup Staff \"one\")", "()", "()"),
        element("(VerticalAxisGroup Above \"\")", "-1", "()"),
        element("(VerticalAxisGroup Staff \"two\")", "()", "()"),
        outer_staff,
        inner_staff,
        element("(VerticalAxisGroup Grouped \"\")", "()", "()"),
        element("(VerticalAxisGroup Staff \"one\")", "()", "()"),
        element("(VerticalAxisGroup Below \"\")", "()", "()"),
        element("(VerticalAxisGroup Lost \"\")", "()", "()"),
        "((TextScript Staff \"one\") (VerticalAxisGroup Staff \"one\"))".to_string(),
        "((TextScript Staff \"two\") (VerticalAxisGroup Staff \"two\"))".to_string(),
        "((TextScript Staff \"\") (VerticalAxisGroup Staff \"\"))".to_string(),
        "((TextScript Staff \"\") (VerticalAxisGroup Staff \"\"))".to_string(),
        format!("((TextScript Group \"inner\") {inner})"),
        format!("((TextScript Group \"outer\") {outer})"),
        "((TextScript Grouped \"\") (VerticalAxisGroup Grouped \"\"))".to_string(),
        "((TextScript Score \"\") System)".to_string(),
    ];
    expected.extend(records.iter().map(|r| warn(r)));
    assert_eq!(
        show_of(&mut vm, "(kept)"),
        format!("({})", expected.join(" "))
    );
}

/// Global with Clef's, KeySignature's, TimeSignature's, BarLine's,
/// BarNumber's, LeftEdge's, BreakAlignGroup's, BreakAlignment's and the
/// columns' descriptions; a Score with `Paper_column_engraver`,
/// `Break_align_engraver` and `Break_probe`; a Plain and a Staff, each
/// with `Break_maker` and `createSpacing` `#f`.
const BREAK_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((Clef (break-align-symbol . clef) (non-musical . #t)
              (meta (name . Clef) (classes Item)
                    (interfaces grob-interface break-aligned-interface clef-interface)))
        (KeySignature (break-align-symbol . key-signature) (non-musical . #t)
                      (meta (name . KeySignature) (classes Item)
                            (interfaces grob-interface break-aligned-interface
                                        key-signature-interface)))
        (TimeSignature (break-align-symbol . time-signature) (non-musical . #t)
                       (meta (name . TimeSignature) (classes Item)
                             (interfaces grob-interface break-aligned-interface
                                         time-signature-interface)))
        (BarLine (break-align-symbol . staff-bar) (non-musical . #t)
                 (meta (name . BarLine) (classes Item)
                       (interfaces grob-interface bar-line-interface break-aligned-interface)))
        (BarNumber (non-musical . #t)
                   (meta (name . BarNumber) (classes Item)
                         (interfaces grob-interface bar-number-interface
                                     break-alignable-interface)))
        (LeftEdge (break-align-symbol . left-edge) (non-musical . #t)
                  (meta (name . LeftEdge) (classes Item)
                        (interfaces grob-interface break-aligned-interface)))
        (BreakAlignGroup (axes 0)
                         (meta (name . BreakAlignGroup) (classes Item)
                               (interfaces grob-interface axis-group-interface
                                           break-aligned-interface)))
        (BreakAlignment (axes 0) (non-musical . #t)
                        (meta (name . BreakAlignment) (classes Item)
                              (interfaces grob-interface axis-group-interface
                                          break-alignment-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Plain) (accepts Staff)
      (consists Paper_column_engraver) (consists Break_align_engraver) (consists Break_probe))",
    "((context-name Plain) (translator-type Engraver_group) (consists Break_maker)
      (assign createSpacing #f))",
    "((context-name Staff) (translator-type Engraver_group) (consists Break_maker)
      (assign createSpacing #f))",
];

/// The probe's `records.scm`, its maker's plan and its probe, written as
/// its `probe.ly` writes them, each record kept in `probed`.
const BREAK_PROBES: &str = "
    (set-object-property! 'createSpacing 'translation-type? boolean?)
    (set-object-property! 'non-musical 'backend-type? boolean?)
    (set-object-property! 'break-align-symbol 'backend-type? symbol?)
    (define-public X 0)
    (define-public Y 1)
    ;; records.scm
    (define labels '())
    (define (gname g) (if (ly:grob? g) (assq-ref (ly:grob-property g 'meta) 'name) g))
    (define (label g)
      (cond ((not (ly:grob? g)) g)
            ((assq-ref labels g) (assq-ref labels g))
            ((memq (gname g) '(PaperColumn NonMusicalPaperColumn))
             (list (gname g) (ly:moment-main (ly:grob-property g 'when))))
            (else (gname g))))
    (define (labelled! g id now)
      (if (not (assq-ref labels g))
          (set! labels
                (acons g (if (eq? (gname g) 'BreakAlignGroup)
                             (list (gname g) (ly:grob-property-data g 'break-align-symbol) now)
                             (list (gname g) id now))
                       labels))))
    (define (pname p)
      (cond ((eq? p ly:grob::x-parent-positioning) 'x-parent-positioning)
            ((eq? p ly:grob::y-parent-positioning) 'y-parent-positioning)
            ((procedure? p) 'proc)
            (else p)))
    (define (rel me other) (cond ((null? other) '()) ((eq? me other) 'me) (else (label other))))
    (define (state g)
      (list (label g) (label (ly:grob-parent g X)) (label (ly:grob-parent g Y))
            (label (ly:grob-object g 'axis-group-parent-X))
            (label (ly:grob-object g 'axis-group-parent-Y))))
    (define (aligned me)
      (let ((group? (eq? (gname me) 'BreakAlignGroup)))
        (cons* (label me) (label (ly:grob-property me 'cause))
               (ly:grob-property-data me 'break-align-symbol) (ly:grob-property-data me 'axes)
               (label (ly:grob-parent me X)) (label (ly:grob-parent me Y))
               (map (lambda (e)
                      (append
                       (list (label e))
                       (if group?
                           '()
                           (list (ly:grob-property-data e 'break-align-symbol)
                                 (pname (ly:grob-property-data e 'X-offset))
                                 (pname (ly:grob-property-data e 'Y-offset))))
                       (list (rel me (ly:grob-parent e X)) (rel me (ly:grob-parent e Y))
                             (rel me (ly:grob-object e 'axis-group-parent-X))
                             (rel me (ly:grob-object e 'axis-group-parent-Y)))))
                    (let ((es (ly:grob-object me 'elements)))
                      (if (ly:grob-array? es) (ly:grob-array->list es) es))))))
    ;; probe.ly
    (define-public probed '())
    (define (now context) (ly:moment-main (ly:context-current-moment context)))
    (define (out x) (set! probed (cons x probed)))
    (define plan
      '(((\"b\" . 0) (BarNumber b-number) (Clef b-clef) (Clef b-musical musical)
                   (KeySignature b-parented (parent . b-musical)) (Clef b-nosym no-symbol)
                   (KeySignature b-key) (TimeSignature b-under (parent . b-clef)))
        ((\"a\" . 0) (Clef a-clef) (KeySignature a-key) (Clef a-clef-2))
        ((\"b\" . 1/4) (KeySignature b-key-2) (TimeSignature b-time) (KeySignature b-key-3))
        ((\"a\" . 1/2) (BarLine a-bar) (BarNumber a-number) (BarNumber a-parented (parent . a-bar))
                     (BarLine a-musical musical))
        ((\"b\" . 3/4) (BarNumber b-number-2))
        ((\"a\" . 3/4) (Clef a-nosym no-symbol))
        ((\"b\" . 1) (Clef b-musical-2 musical) (Clef b-nosym-2 no-symbol)
                   (BarNumber b-number-musical musical) (Clef b-parented-2 (parent . b-musical-2)))
        ((\"a\" . 1) (Clef a-musical-2 musical) (Clef a-parented-2 (parent . a-musical-2)))))
    ;; At each moment, the grobs of `plan` for the context's id, each a
    ;; type, a tag and options: `musical` (non-musical #f), `no-symbol`
    ;; (break-align-symbol '()) or (parent . tag), an X parent made before.
    (ly:register-translator
      (lambda (context)
        `((process-music
           . ,(lambda (engraver)
                (let ((made '()))
                  (for-each
                   (lambda (spec)
                     (let ((g (ly:engraver-make-grob engraver (car spec) '())))
                       (set! labels (acons g (cadr spec) labels))
                       (set! made (acons (cadr spec) g made))
                       (for-each
                        (lambda (o)
                          (cond ((eq? o 'musical) (ly:grob-set-property! g 'non-musical #f))
                                ((eq? o 'no-symbol) (ly:grob-set-property! g 'break-align-symbol '()))
                                (else (ly:grob-set-parent! g X (assq-ref made (cdr o))))))
                        (cddr spec))))
                   (or (assoc-ref plan (cons (ly:context-id context) (now context))) '())))))))
      'Break_maker)
    ;; Each grob acknowledged, labelled, and a LeftEdge's cause after it;
    ;; at the stop, the parents and
    ;; axis-group parents of those of the timestep; at the finalize, each
    ;; BreakAlignment and BreakAlignGroup.
    (ly:register-translator
      (lambda (context)
        (let ((step '()) (aligns '()))
          (define (note g source)
            (labelled! g (ly:context-id (ly:translator-context source)) (now context))
            (out (list 'acknowledged (label g)))
            (if (eq? (gname g) 'LeftEdge)
                (out (list 'cause (label g) (label (ly:grob-property g 'cause)))))
            (set! step (cons g step))
            (if (memq (gname g) '(BreakAlignment BreakAlignGroup))
                (set! aligns (cons g aligns))))
          `((acknowledgers
             (break-aligned-interface . ,(lambda (t g s) (note g s)))
             (break-alignable-interface . ,(lambda (t g s) (note g s)))
             (break-alignment-interface . ,(lambda (t g s) (note g s))))
            (stop-translation-timestep
             . ,(lambda (t)
                  (if (pair? step) (out (cons* 'stop (now context) (map state (reverse step)))))
                  (set! step '())))
            (finalize
             . ,(lambda (t) (for-each (lambda (a) (out (aligned a))) (reverse aligns)))))))
      'Break_probe)
    (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
    (define (s) (make-music 'SkipEvent 'duration (ly:make-duration 2)))
    (define (new type m)
      (make-music 'ContextSpeccedMusic 'context-type type 'create-new #t 'element m))
    (define (new-id type id m)
      (make-music 'ContextSpeccedMusic 'context-type type 'context-id id 'create-new #t
                  'element m))";

/// `Break_align_engraver` (MS3 step 21, `break-align-engraver.cc`),
/// layout's class found before the music crate's placeholder: a
/// break-aligned item with no X parent, non-musical and whose
/// `break-align-symbol` is a symbol joins the BreakAlignGroup of its
/// symbol in the timestep's BreakAlignment, each made for the first that
/// needs it (a group with that item as cause, the alignment with none);
/// a break-alignable one with no X parent and non-musical takes the
/// alignment, made if needed, as X parent; others are left as they are,
/// and a timestep with none of either has no alignment. A group's X and Y
/// parent and `axis-group-parent-X` are the alignment; an item's X parent
/// is its group, and so is its `axis-group-parent-X` until
/// `Score_engraver` typesets it into the System. As
/// LilyPond 2.26.0 gives the probe's score, before its "Preprocessing"
/// (`$LILY_CORPUS/device-runs/2026-10-11-ms3-step21/session4-probe/`,
/// `probe.out`, with `createSpacing` `#f`: no LeftEdge).
#[test]
fn break_aligned_grobs_join_the_groups_of_their_symbols_as_lilypond_does() {
    use lily_engine_layout::break_align_engraver::BREAK_ALIGN_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(BREAK_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, BREAK_PROBES);
    let music = eval(
        &mut vm,
        "(new 'Score (par
          (new-id 'Plain \"b\" (seq (s) (s) (s) (s) (s)))
          (new-id 'Staff \"a\" (seq (s) (s) (s) (s) (s)))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &BREAK_ALIGN_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(show_of(&mut vm, "(kept)"), "()");
    assert_eq!(
        show_of(&mut vm, "(reverse probed)"),
        "((acknowledged b-number) \
          (acknowledged b-clef) \
          (acknowledged b-musical) \
          (acknowledged b-parented) \
          (acknowledged b-nosym) \
          (acknowledged b-key) \
          (acknowledged b-under) \
          (acknowledged a-clef) \
          (acknowledged a-key) \
          (acknowledged a-clef-2) \
          (acknowledged (BreakAlignment \"\" 0)) \
          (acknowledged (BreakAlignGroup clef 0)) \
          (acknowledged \
           (BreakAlignGroup key-signature 0)) \
          (stop 0 \
           (b-number (BreakAlignment \"\" 0) () (NonMusicalPaperColumn 0) ()) \
           (b-clef (BreakAlignGroup clef 0) () (BreakAlignGroup clef 0) ()) \
           (b-musical (PaperColumn 0) () (PaperColumn 0) ()) \
           (b-parented b-musical () (PaperColumn 0) ()) \
           (b-nosym (NonMusicalPaperColumn 0) () (NonMusicalPaperColumn 0) ()) \
           (b-key (BreakAlignGroup key-signature 0) () (BreakAlignGroup key-signature 0) ()) \
           (b-under b-clef () (NonMusicalPaperColumn 0) ()) \
           (a-clef (BreakAlignGroup clef 0) () (BreakAlignGroup clef 0) ()) \
           (a-key (BreakAlignGroup key-signature 0) () (BreakAlignGroup key-signature 0) ()) \
           (a-clef-2 (BreakAlignGroup clef 0) () (BreakAlignGroup clef 0) ()) \
           ((BreakAlignment \"\" 0) (NonMusicalPaperColumn 0) () (NonMusicalPaperColumn 0) ()) \
           ((BreakAlignGroup clef 0) (BreakAlignment \"\" 0) (BreakAlignment \"\" 0) \
            (BreakAlignment \"\" 0) ()) \
           ((BreakAlignGroup key-signature 0) (BreakAlignment \"\" 0) (BreakAlignment \"\" 0) \
            (BreakAlignment \"\" 0) ())) \
          (acknowledged b-key-2) \
          (acknowledged b-time) \
          (acknowledged b-key-3) \
          (acknowledged (BreakAlignment \"\" 1/4)) \
          (acknowledged \
           (BreakAlignGroup key-signature 1/4)) \
          (acknowledged \
           (BreakAlignGroup time-signature 1/4)) \
          (stop 1/4 \
           (b-key-2 (BreakAlignGroup key-signature 1/4) () (BreakAlignGroup key-signature 1/4) \
            ()) \
           (b-time (BreakAlignGroup time-signature 1/4) () (BreakAlignGroup time-signature 1/4) \
            ()) \
           (b-key-3 (BreakAlignGroup key-signature 1/4) () (BreakAlignGroup key-signature 1/4) \
            ()) \
           ((BreakAlignment \"\" 1/4) (NonMusicalPaperColumn 1/4) () (NonMusicalPaperColumn \
            1/4) ()) \
           ((BreakAlignGroup key-signature 1/4) (BreakAlignment \"\" 1/4) (BreakAlignment \"\" \
            1/4) (BreakAlignment \"\" 1/4) ()) \
           ((BreakAlignGroup time-signature 1/4) (BreakAlignment \"\" 1/4) (BreakAlignment \"\" \
            1/4) (BreakAlignment \"\" 1/4) ())) \
          (acknowledged a-bar) \
          (acknowledged a-number) \
          (acknowledged a-parented) \
          (acknowledged a-musical) \
          (acknowledged (BreakAlignment \"\" 1/2)) \
          (acknowledged (BreakAlignGroup staff-bar 1/2)) \
          (stop 1/2 \
           (a-bar (BreakAlignGroup staff-bar 1/2) () (BreakAlignGroup staff-bar 1/2) ()) \
           (a-number (BreakAlignment \"\" 1/2) () (NonMusicalPaperColumn 1/2) ()) \
           (a-parented a-bar () (NonMusicalPaperColumn 1/2) ()) \
           (a-musical (PaperColumn 1/2) () (PaperColumn 1/2) ()) \
           ((BreakAlignment \"\" 1/2) (NonMusicalPaperColumn 1/2) () (NonMusicalPaperColumn \
            1/2) ()) \
           ((BreakAlignGroup staff-bar 1/2) (BreakAlignment \"\" 1/2) (BreakAlignment \"\" 1/2) \
            (BreakAlignment \"\" 1/2) ())) \
          (acknowledged b-number-2) \
          (acknowledged a-nosym) \
          (acknowledged (BreakAlignment \"\" 3/4)) \
          (stop 3/4 \
           (b-number-2 (BreakAlignment \"\" 3/4) () (NonMusicalPaperColumn 3/4) ()) \
           (a-nosym (NonMusicalPaperColumn 3/4) () (NonMusicalPaperColumn 3/4) ()) \
           ((BreakAlignment \"\" 3/4) (NonMusicalPaperColumn 3/4) () (NonMusicalPaperColumn \
            3/4) ())) \
          (acknowledged b-musical-2) \
          (acknowledged b-nosym-2) \
          (acknowledged b-number-musical) \
          (acknowledged b-parented-2) \
          (acknowledged a-musical-2) \
          (acknowledged a-parented-2) \
          (stop 1 \
           (b-musical-2 (PaperColumn 1) () (PaperColumn 1) ()) \
           (b-nosym-2 (NonMusicalPaperColumn 1) () (NonMusicalPaperColumn 1) ()) \
           (b-number-musical (PaperColumn 1) () (PaperColumn 1) ()) \
           (b-parented-2 b-musical-2 () (PaperColumn 1) ()) \
           (a-musical-2 (PaperColumn 1) () (PaperColumn 1) ()) \
           (a-parented-2 a-musical-2 () (PaperColumn 1) ())) \
          ((BreakAlignment \"\" 0) () () (0) (NonMusicalPaperColumn 0) System \
           ((BreakAlignGroup clef 0) clef x-parent-positioning () me me me ()) \
           ((BreakAlignGroup key-signature 0) key-signature x-parent-positioning () me me me \
            ())) \
          ((BreakAlignGroup clef 0) b-clef clef (0) (BreakAlignment \"\" 0) (BreakAlignment \
            \"\" 0) \
           (b-clef me System System System) \
           (a-clef me System System System) \
           (a-clef-2 me System System System)) \
          ((BreakAlignGroup key-signature 0) b-key key-signature (0) (BreakAlignment \"\" 0) \
            (BreakAlignment \"\" 0) (b-key me System System System) (a-key me System System \
            System)) \
          ((BreakAlignment \"\" 1/4) () () (0) (NonMusicalPaperColumn 1/4) System \
           ((BreakAlignGroup key-signature 1/4) key-signature x-parent-positioning () me me me \
            ()) \
           ((BreakAlignGroup time-signature 1/4) time-signature x-parent-positioning () me me \
            me ())) \
          ((BreakAlignGroup key-signature 1/4) b-key-2 key-signature (0) (BreakAlignment \"\" \
            1/4) (BreakAlignment \"\" 1/4) \
           (b-key-2 me System System System) \
           (b-key-3 me System System System)) \
          ((BreakAlignGroup time-signature 1/4) b-time time-signature (0) (BreakAlignment \"\" \
            1/4) (BreakAlignment \"\" 1/4) \
           (b-time me System System System)) \
          ((BreakAlignment \"\" 1/2) () () (0) (NonMusicalPaperColumn 1/2) System \
           ((BreakAlignGroup staff-bar 1/2) staff-bar x-parent-positioning () me me me ())) \
          ((BreakAlignGroup staff-bar 1/2) a-bar staff-bar (0) (BreakAlignment \"\" 1/2) \
            (BreakAlignment \"\" 1/2) (a-bar me System System System)) \
          ((BreakAlignment \"\" 3/4) () () (0) (NonMusicalPaperColumn 3/4) System))"
    );
}

/// `Break_align_engraver`'s LeftEdge (MS3 step 21,
/// `break-align-engraver.cc:117–128`): before the timestep's first
/// break-aligned item that is not skipped joins its group, if
/// `createSpacing` is `#t` (nothing else counts: `'()` passes the type
/// check and is not true) in the context of the engraver that made it, a
/// LeftEdge made as that engraver makes one, with no cause, joins the
/// group of its `break-align-symbol` (left-edge), that group's cause.
/// Acknowledged after the alignment and before its group (its cause `()`,
/// written after it), it is skipped.
/// So "a"'s at 0, 1/2 and 5/4 (after b's skipped clef), "b"'s at 3/2 and
/// none at 7/4. As LilyPond 2.26.0 gives the probe's score, before its
/// "Preprocessing"
/// (`$LILY_CORPUS/device-runs/2026-10-11-ms3-step21/session5-probe/`,
/// `probe-left-edge.out`: `probe.ly` with the Staff's `createSpacing`
/// `#t` and its `extended` timesteps).
#[test]
fn the_left_edge_comes_from_the_first_break_aligned_grob_as_lilypond_does() {
    use lily_engine_layout::break_align_engraver::BREAK_ALIGN_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    let staff = "((context-name Staff) (translator-type Engraver_group) (consists Break_maker)
      (assign createSpacing #t))";
    let Some((mut vm, layout)) = vm_with(&[BREAK_DEFS[0], BREAK_DEFS[1], BREAK_DEFS[2], staff])
    else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, BREAK_PROBES);
    eval(
        &mut vm,
        "(set! plan
           (append plan
                   '(((\"b\" . 5/4) (Clef b-musical-3 musical))
                     ((\"a\" . 5/4) (KeySignature a-key-3))
                     ((\"b\" . 3/2) (BarLine b-bar))
                     ((\"a\" . 3/2) (Clef a-clef-3))
                     ((\"a\" . 7/4) (Clef a-clef-4)))))
         (define (set value) (make-music 'PropertySet 'symbol 'createSpacing 'value value))",
    );
    let music = eval(
        &mut vm,
        "(new 'Score (par
          (new-id 'Plain \"b\" (seq (s) (s) (s) (s) (s) (set #t) (s) (s) (set #f) (s)))
          (new-id 'Staff \"a\" (seq (s) (s) (s) (s) (s) (s) (set #f) (s) (set '()) (s)))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &BREAK_ALIGN_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(show_of(&mut vm, "(kept)"), "()");
    assert_eq!(
        show_of(&mut vm, "(reverse probed)"),
        "((acknowledged b-number) \
          (acknowledged b-clef) \
          (acknowledged b-musical) \
          (acknowledged b-parented) \
          (acknowledged b-nosym) \
          (acknowledged b-key) \
          (acknowledged b-under) \
          (acknowledged a-clef) \
          (acknowledged a-key) \
          (acknowledged a-clef-2) \
          (acknowledged (BreakAlignment \"\" 0)) \
          (acknowledged (BreakAlignGroup clef 0)) \
          (acknowledged (BreakAlignGroup key-signature 0)) \
          (acknowledged (LeftEdge \"a\" 0)) \
          (cause (LeftEdge \"a\" 0) ()) \
          (acknowledged (BreakAlignGroup left-edge 0)) \
          (stop 0 \
           (b-number (BreakAlignment \"\" 0) () (NonMusicalPaperColumn 0) ()) \
           (b-clef (BreakAlignGroup clef 0) () (BreakAlignGroup clef 0) ()) \
           (b-musical (PaperColumn 0) () (PaperColumn 0) ()) \
           (b-parented b-musical () (PaperColumn 0) ()) \
           (b-nosym (NonMusicalPaperColumn 0) () (NonMusicalPaperColumn 0) ()) \
           (b-key (BreakAlignGroup key-signature 0) () (BreakAlignGroup key-signature 0) ()) \
           (b-under b-clef () (NonMusicalPaperColumn 0) ()) \
           (a-clef (BreakAlignGroup clef 0) () (BreakAlignGroup clef 0) ()) \
           (a-key (BreakAlignGroup key-signature 0) () (BreakAlignGroup key-signature 0) ()) \
           (a-clef-2 (BreakAlignGroup clef 0) () (BreakAlignGroup clef 0) ()) \
           ((BreakAlignment \"\" 0) (NonMusicalPaperColumn 0) () (NonMusicalPaperColumn 0) ()) \
           ((BreakAlignGroup clef 0) (BreakAlignment \"\" 0) (BreakAlignment \"\" 0) \
            (BreakAlignment \"\" 0) ()) \
           ((BreakAlignGroup key-signature 0) (BreakAlignment \"\" 0) (BreakAlignment \"\" 0) \
            (BreakAlignment \"\" 0) ()) \
           ((LeftEdge \"a\" 0) (BreakAlignGroup left-edge 0) () (BreakAlignGroup left-edge 0) ()) \
           ((BreakAlignGroup left-edge 0) (BreakAlignment \"\" 0) (BreakAlignment \"\" 0) \
            (BreakAlignment \"\" 0) ())) \
          (acknowledged b-key-2) \
          (acknowledged b-time) \
          (acknowledged b-key-3) \
          (acknowledged (BreakAlignment \"\" 1/4)) \
          (acknowledged (BreakAlignGroup key-signature 1/4)) \
          (acknowledged (BreakAlignGroup time-signature 1/4)) \
          (stop 1/4 \
           (b-key-2 (BreakAlignGroup key-signature 1/4) () (BreakAlignGroup key-signature 1/4) \
            ()) \
           (b-time (BreakAlignGroup time-signature 1/4) () (BreakAlignGroup time-signature 1/4) \
            ()) \
           (b-key-3 (BreakAlignGroup key-signature 1/4) () (BreakAlignGroup key-signature 1/4) \
            ()) \
           ((BreakAlignment \"\" 1/4) (NonMusicalPaperColumn 1/4) () (NonMusicalPaperColumn 1/4) \
            ()) \
           ((BreakAlignGroup key-signature 1/4) (BreakAlignment \"\" 1/4) (BreakAlignment \"\" \
            1/4) (BreakAlignment \"\" 1/4) ()) \
           ((BreakAlignGroup time-signature 1/4) (BreakAlignment \"\" 1/4) (BreakAlignment \"\" \
            1/4) (BreakAlignment \"\" 1/4) ())) \
          (acknowledged a-bar) \
          (acknowledged a-number) \
          (acknowledged a-parented) \
          (acknowledged a-musical) \
          (acknowledged (BreakAlignment \"\" 1/2)) \
          (acknowledged (LeftEdge \"a\" 1/2)) \
          (cause (LeftEdge \"a\" 1/2) ()) \
          (acknowledged (BreakAlignGroup left-edge 1/2)) \
          (acknowledged (BreakAlignGroup staff-bar 1/2)) \
          (stop 1/2 \
           (a-bar (BreakAlignGroup staff-bar 1/2) () (BreakAlignGroup staff-bar 1/2) ()) \
           (a-number (BreakAlignment \"\" 1/2) () (NonMusicalPaperColumn 1/2) ()) \
           (a-parented a-bar () (NonMusicalPaperColumn 1/2) ()) \
           (a-musical (PaperColumn 1/2) () (PaperColumn 1/2) ()) \
           ((BreakAlignment \"\" 1/2) (NonMusicalPaperColumn 1/2) () (NonMusicalPaperColumn 1/2) \
            ()) \
           ((LeftEdge \"a\" 1/2) (BreakAlignGroup left-edge 1/2) () (BreakAlignGroup left-edge \
            1/2) ()) \
           ((BreakAlignGroup left-edge 1/2) (BreakAlignment \"\" 1/2) (BreakAlignment \"\" 1/2) \
            (BreakAlignment \"\" 1/2) ()) \
           ((BreakAlignGroup staff-bar 1/2) (BreakAlignment \"\" 1/2) (BreakAlignment \"\" 1/2) \
            (BreakAlignment \"\" 1/2) ())) \
          (acknowledged b-number-2) \
          (acknowledged a-nosym) \
          (acknowledged (BreakAlignment \"\" 3/4)) \
          (stop 3/4 \
           (b-number-2 (BreakAlignment \"\" 3/4) () (NonMusicalPaperColumn 3/4) ()) \
           (a-nosym (NonMusicalPaperColumn 3/4) () (NonMusicalPaperColumn 3/4) ()) \
           ((BreakAlignment \"\" 3/4) (NonMusicalPaperColumn 3/4) () (NonMusicalPaperColumn 3/4) \
            ())) \
          (acknowledged b-musical-2) \
          (acknowledged b-nosym-2) \
          (acknowledged b-number-musical) \
          (acknowledged b-parented-2) \
          (acknowledged a-musical-2) \
          (acknowledged a-parented-2) \
          (stop 1 \
           (b-musical-2 (PaperColumn 1) () (PaperColumn 1) ()) \
           (b-nosym-2 (NonMusicalPaperColumn 1) () (NonMusicalPaperColumn 1) ()) \
           (b-number-musical (PaperColumn 1) () (PaperColumn 1) ()) \
           (b-parented-2 b-musical-2 () (PaperColumn 1) ()) \
           (a-musical-2 (PaperColumn 1) () (PaperColumn 1) ()) \
           (a-parented-2 a-musical-2 () (PaperColumn 1) ())) \
          (acknowledged b-musical-3) \
          (acknowledged a-key-3) \
          (acknowledged (BreakAlignment \"\" 5/4)) \
          (acknowledged (LeftEdge \"a\" 5/4)) \
          (cause (LeftEdge \"a\" 5/4) ()) \
          (acknowledged (BreakAlignGroup left-edge 5/4)) \
          (acknowledged (BreakAlignGroup key-signature 5/4)) \
          (stop 5/4 \
           (b-musical-3 (PaperColumn 5/4) () (PaperColumn 5/4) ()) \
           (a-key-3 (BreakAlignGroup key-signature 5/4) () (BreakAlignGroup key-signature 5/4) \
            ()) \
           ((BreakAlignment \"\" 5/4) (NonMusicalPaperColumn 5/4) () (NonMusicalPaperColumn 5/4) \
            ()) \
           ((LeftEdge \"a\" 5/4) (BreakAlignGroup left-edge 5/4) () (BreakAlignGroup left-edge \
            5/4) ()) \
           ((BreakAlignGroup left-edge 5/4) (BreakAlignment \"\" 5/4) (BreakAlignment \"\" 5/4) \
            (BreakAlignment \"\" 5/4) ()) \
           ((BreakAlignGroup key-signature 5/4) (BreakAlignment \"\" 5/4) (BreakAlignment \"\" \
            5/4) (BreakAlignment \"\" 5/4) ())) \
          (acknowledged b-bar) \
          (acknowledged a-clef-3) \
          (acknowledged (BreakAlignment \"\" 3/2)) \
          (acknowledged (LeftEdge \"b\" 3/2)) \
          (cause (LeftEdge \"b\" 3/2) ()) \
          (acknowledged (BreakAlignGroup left-edge 3/2)) \
          (acknowledged (BreakAlignGroup staff-bar 3/2)) \
          (acknowledged (BreakAlignGroup clef 3/2)) \
          (stop 3/2 \
           (b-bar (BreakAlignGroup staff-bar 3/2) () (BreakAlignGroup staff-bar 3/2) ()) \
           (a-clef-3 (BreakAlignGroup clef 3/2) () (BreakAlignGroup clef 3/2) ()) \
           ((BreakAlignment \"\" 3/2) (NonMusicalPaperColumn 3/2) () (NonMusicalPaperColumn 3/2) \
            ()) \
           ((LeftEdge \"b\" 3/2) (BreakAlignGroup left-edge 3/2) () (BreakAlignGroup left-edge \
            3/2) ()) \
           ((BreakAlignGroup left-edge 3/2) (BreakAlignment \"\" 3/2) (BreakAlignment \"\" 3/2) \
            (BreakAlignment \"\" 3/2) ()) \
           ((BreakAlignGroup staff-bar 3/2) (BreakAlignment \"\" 3/2) (BreakAlignment \"\" 3/2) \
            (BreakAlignment \"\" 3/2) ()) \
           ((BreakAlignGroup clef 3/2) (BreakAlignment \"\" 3/2) (BreakAlignment \"\" 3/2) \
            (BreakAlignment \"\" 3/2) ())) \
          (acknowledged a-clef-4) \
          (acknowledged (BreakAlignment \"\" 7/4)) \
          (acknowledged (BreakAlignGroup clef 7/4)) \
          (stop 7/4 \
           (a-clef-4 (BreakAlignGroup clef 7/4) () (BreakAlignGroup clef 7/4) ()) \
           ((BreakAlignment \"\" 7/4) (NonMusicalPaperColumn 7/4) () (NonMusicalPaperColumn 7/4) \
            ()) \
           ((BreakAlignGroup clef 7/4) (BreakAlignment \"\" 7/4) (BreakAlignment \"\" 7/4) \
            (BreakAlignment \"\" 7/4) ())) \
          ((BreakAlignment \"\" 0) () () (0) (NonMusicalPaperColumn 0) System \
           ((BreakAlignGroup clef 0) clef x-parent-positioning () me me me ()) \
           ((BreakAlignGroup key-signature 0) key-signature x-parent-positioning () me me me ()) \
           ((BreakAlignGroup left-edge 0) left-edge x-parent-positioning () me me me ())) \
          ((BreakAlignGroup clef 0) b-clef clef (0) (BreakAlignment \"\" 0) (BreakAlignment \"\" \
           0) \
           (b-clef me System System System) \
           (a-clef me System System System) \
           (a-clef-2 me System System System)) \
          ((BreakAlignGroup key-signature 0) b-key key-signature (0) (BreakAlignment \"\" 0) \
           (BreakAlignment \"\" 0) \
           (b-key me System System System) \
           (a-key me System System System)) \
          ((BreakAlignGroup left-edge 0) (LeftEdge \"a\" 0) left-edge (0) (BreakAlignment \"\" 0) \
           (BreakAlignment \"\" 0) \
           ((LeftEdge \"a\" 0) me System System System)) \
          ((BreakAlignment \"\" 1/4) () () (0) (NonMusicalPaperColumn 1/4) System \
           ((BreakAlignGroup key-signature 1/4) key-signature x-parent-positioning () me me me \
            ()) \
           ((BreakAlignGroup time-signature 1/4) time-signature x-parent-positioning () me me me \
            ())) \
          ((BreakAlignGroup key-signature 1/4) b-key-2 key-signature (0) (BreakAlignment \"\" \
           1/4) (BreakAlignment \"\" 1/4) \
           (b-key-2 me System System System) \
           (b-key-3 me System System System)) \
          ((BreakAlignGroup time-signature 1/4) b-time time-signature (0) (BreakAlignment \"\" \
           1/4) (BreakAlignment \"\" 1/4) \
           (b-time me System System System)) \
          ((BreakAlignment \"\" 1/2) () () (0) (NonMusicalPaperColumn 1/2) System \
           ((BreakAlignGroup left-edge 1/2) left-edge x-parent-positioning () me me me ()) \
           ((BreakAlignGroup staff-bar 1/2) staff-bar x-parent-positioning () me me me ())) \
          ((BreakAlignGroup left-edge 1/2) (LeftEdge \"a\" 1/2) left-edge (0) (BreakAlignment \
           \"\" 1/2) (BreakAlignment \"\" 1/2) \
           ((LeftEdge \"a\" 1/2) me System System System)) \
          ((BreakAlignGroup staff-bar 1/2) a-bar staff-bar (0) (BreakAlignment \"\" 1/2) \
           (BreakAlignment \"\" 1/2) \
           (a-bar me System System System)) \
          ((BreakAlignment \"\" 3/4) () () (0) (NonMusicalPaperColumn 3/4) System) \
          ((BreakAlignment \"\" 5/4) () () (0) (NonMusicalPaperColumn 5/4) System \
           ((BreakAlignGroup left-edge 5/4) left-edge x-parent-positioning () me me me ()) \
           ((BreakAlignGroup key-signature 5/4) key-signature x-parent-positioning () me me me \
            ())) \
          ((BreakAlignGroup left-edge 5/4) (LeftEdge \"a\" 5/4) left-edge (0) (BreakAlignment \
           \"\" 5/4) (BreakAlignment \"\" 5/4) \
           ((LeftEdge \"a\" 5/4) me System System System)) \
          ((BreakAlignGroup key-signature 5/4) a-key-3 key-signature (0) (BreakAlignment \"\" \
           5/4) (BreakAlignment \"\" 5/4) \
           (a-key-3 me System System System)) \
          ((BreakAlignment \"\" 3/2) () () (0) (NonMusicalPaperColumn 3/2) System \
           ((BreakAlignGroup left-edge 3/2) left-edge x-parent-positioning () me me me ()) \
           ((BreakAlignGroup staff-bar 3/2) staff-bar x-parent-positioning () me me me ()) \
           ((BreakAlignGroup clef 3/2) clef x-parent-positioning () me me me ())) \
          ((BreakAlignGroup left-edge 3/2) (LeftEdge \"b\" 3/2) left-edge (0) (BreakAlignment \
           \"\" 3/2) (BreakAlignment \"\" 3/2) \
           ((LeftEdge \"b\" 3/2) me System System System)) \
          ((BreakAlignGroup staff-bar 3/2) b-bar staff-bar (0) (BreakAlignment \"\" 3/2) \
           (BreakAlignment \"\" 3/2) \
           (b-bar me System System System)) \
          ((BreakAlignGroup clef 3/2) a-clef-3 clef (0) (BreakAlignment \"\" 3/2) (BreakAlignment \
           \"\" 3/2) \
           (a-clef-3 me System System System)) \
          ((BreakAlignment \"\" 7/4) () () (0) (NonMusicalPaperColumn 7/4) System \
           ((BreakAlignGroup clef 7/4) clef x-parent-positioning () me me me ())) \
          ((BreakAlignGroup clef 7/4) a-clef-4 clef (0) (BreakAlignment \"\" 7/4) (BreakAlignment \
           \"\" 7/4) \
           (a-clef-4 me System System System)))"
    );
}

/// Global with Clef's, KeySignature's, TimeSignature's and BarLine's
/// descriptions (each with `pure-from-neighbor-interface`), BarNumber's,
/// TextScript's, TextSpanner's and the columns'; a Score with
/// `Paper_column_engraver` and `Pure_probe`; a Staff with `Pure_maker` and
/// `Pure_from_neighbor_engraver`, its Voice and a Plain each with
/// `Pure_maker`.
const PURE_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((Clef (non-musical . #t)
              (meta (name . Clef) (classes Item)
                    (interfaces grob-interface clef-interface pure-from-neighbor-interface)))
        (KeySignature (non-musical . #t)
                      (meta (name . KeySignature) (classes Item)
                            (interfaces grob-interface key-signature-interface
                                        pure-from-neighbor-interface)))
        (TimeSignature (non-musical . #t)
                       (meta (name . TimeSignature) (classes Item)
                             (interfaces grob-interface time-signature-interface
                                         pure-from-neighbor-interface)))
        (BarLine (non-musical . #t)
                 (meta (name . BarLine) (classes Item)
                       (interfaces grob-interface bar-line-interface
                                   pure-from-neighbor-interface)))
        (BarNumber (non-musical . #t)
                   (meta (name . BarNumber) (classes Item)
                         (interfaces grob-interface bar-number-interface)))
        (TextScript (meta (name . TextScript) (classes Item)
                          (interfaces grob-interface text-script-interface)))
        (TextSpanner (meta (name . TextSpanner) (classes Spanner)
                           (interfaces grob-interface line-spanner-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff) (accepts Plain)
      (consists Paper_column_engraver) (consists Pure_probe))",
    "((context-name Staff) (translator-type Engraver_group) (accepts Voice)
      (consists Pure_maker) (consists Pure_from_neighbor_engraver))",
    "((context-name Voice) (translator-type Engraver_group) (consists Pure_maker))",
    "((context-name Plain) (translator-type Engraver_group) (consists Pure_maker))",
];

/// The probe's `records.scm`, its maker's plan and its probe, written as
/// its `probe.ly` writes them, each record kept in `probed`.
const PURE_PROBES: &str = "
    (set-object-property! 'non-musical 'backend-type? boolean?)
    (define-public X 0)
    (define-public LEFT -1)
    (define-public RIGHT 1)
    ;; records.scm
    (define labels '())
    (define (gname g) (if (ly:grob? g) (assq-ref (ly:grob-property g 'meta) 'name) g))
    (define (label g)
      (cond ((not (ly:grob? g)) g)
            ((assq-ref labels g) (assq-ref labels g))
            (else (gname g))))
    (define-public probed '())
    (define (out x) (set! probed (cons x probed)))
    (define (arr x)
      (if (ly:grob-array? x) (cons 'array (map label (ly:grob-array->list x))) x))
    (define (neighbored g)
      (let ((r (ly:grob-spanned-column-rank-interval g)))
        (list (label g) (list (car r) (cdr r)) (arr (ly:grob-object g 'neighbors)))))
    ;; probe.ly
    (define (now context) (ly:moment-main (ly:context-current-moment context)))
    (define plan
      '(((\"a\" . 0) (BarNumber a-num-0) (TextScript a-text-0))
        ((\"v\" . 0) (TextScript v-text-0) (TextSpanner v-span spanner))
        ((\"e\" . 0) (TextScript e-text-0))
        ((\"a\" . 1/4) (Clef a-clef-1) (BarNumber a-num-1) (TextScript a-text-1))
        ((\"v\" . 1/4) (Clef v-clef-m musical))
        ((\"v\" . 1/2) (Clef v-clef-free (parent . v-span))
                       (TextScript v-text-free (parent . v-span)))
        ((\"a\" . 3/4) (BarLine a-bar) (TimeSignature a-time) (TextScript a-text-3))
        ((\"p\" . 3/4) (TextScript p-text-3) (Clef p-clef-3))
        ((\"a\" . 1) (BarNumber a-num-4))
        ((\"v\" . 1) (TextScript v-text-4) (KeySignature v-key-late (parent . a-clef-1)))
        ((\"a\" . 5/4) (KeySignature a-key-5) (TextScript a-text-5))
        ((\"a\" . 3/2) (TextScript a-text-6) (TextScript a-late (parent . a-text-0)))))
    (define made '())
    ;; At each moment, the grobs of `plan` for the context's id, each a
    ;; type, a tag and options: `musical` (non-musical #f), `spanner`
    ;; (both bounds the musical column) or (parent . tag), an X parent made
    ;; before.
    (ly:register-translator
      (lambda (context)
        `((process-music
           . ,(lambda (engraver)
                (for-each
                 (lambda (spec)
                   (let ((g (ly:engraver-make-grob engraver (car spec) '())))
                     (set! labels (acons g (cadr spec) labels))
                     (set! made (acons (cadr spec) g made))
                     (for-each
                      (lambda (o)
                        (cond ((eq? o 'musical) (ly:grob-set-property! g 'non-musical #f))
                              ((eq? o 'spanner)
                               (let ((col (ly:context-property context 'currentMusicalColumn)))
                                 (ly:spanner-set-bound! g LEFT col)
                                 (ly:spanner-set-bound! g RIGHT col)))
                              (else (ly:grob-set-parent! g X (assq-ref made (cdr o))))))
                      (cddr spec))))
                 (or (assoc-ref plan (cons (ly:context-id context) (now context))) '()))))))
      'Pure_maker)
    ;; Each grob but the paper columns acknowledged; at the finalize, after
    ;; the Staff's, each one's label, column ranks and `neighbors`.
    (ly:register-translator
      (lambda (context)
        (let ((seen '()))
          `((acknowledgers
             (grob-interface
              . ,(lambda (t g s)
                   (if (not (memq (gname g) '(PaperColumn NonMusicalPaperColumn)))
                       (set! seen (cons g seen))))))
            (finalize
             . ,(lambda (t) (for-each (lambda (g) (out (neighbored g))) (reverse seen)))))))
      'Pure_probe)
    (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
    (define (s) (make-music 'SkipEvent 'duration (ly:make-duration 2)))
    (define (skips) (seq (s) (s) (s) (s) (s) (s) (s) (s)))
    (define (new type m)
      (make-music 'ContextSpeccedMusic 'context-type type 'create-new #t 'element m))
    (define (new-id type id m)
      (make-music 'ContextSpeccedMusic 'context-type type 'context-id id 'create-new #t
                  'element m))";

/// `Pure_from_neighbor_engraver` (MS3 step 21,
/// `pure-from-neighbor-engraver.cc`), layout's class found before the
/// music crate's placeholder: at its finalize, the items of its context and
/// below it with `pure-from-neighbor-interface` clumped by their left
/// column ranks, each other item added to the `neighbors` of the items of
/// the last clump left of it and of the first not left of it, but those in
/// its column; both lists sorted by rank first, so that an item
/// acknowledged late (a key signature on a clef, a text script on an
/// earlier one) is taken by its rank, equal ranks in acknowledgement order.
/// A clump with no item between it and the next keeps `neighbors` unset; a
/// context with none needing neighbors ("e") sets nothing; a spanner, and
/// another context's items ("p"), are no one's neighbors. Items whose X
/// parent is a spanner have no column, their rank interval empty (its
/// left `VPOS`, above its right): they clump last, and such a text script
/// is such a clef's neighbor, since it spans no one column. As LilyPond
/// 2.26.0 gives the probe's score
/// (`$LILY_CORPUS/device-runs/2026-10-11-ms3-step21/session6-probe/`,
/// `probe.out`).
#[test]
fn pure_from_neighbor_engraver_sets_the_neighbors_as_lilypond_does() {
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::pure_from_neighbor_engraver::PURE_FROM_NEIGHBOR_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(PURE_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, PURE_PROBES);
    let music = eval(
        &mut vm,
        "(new 'Score (par
          (new-id 'Staff \"a\" (par (skips) (new-id 'Voice \"v\" (skips))))
          (new-id 'Staff \"e\" (skips))
          (new-id 'Plain \"p\" (skips))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &PURE_FROM_NEIGHBOR_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(show_of(&mut vm, "(kept)"), "()");
    assert_eq!(
        show_of(&mut vm, "(reverse probed)"),
        "((v-text-0 (1 1) ()) \
          (v-span (1 1) ()) \
          (a-num-0 (0 0) ()) \
          (a-text-0 (1 1) ()) \
          (e-text-0 (1 1) ()) \
          (v-clef-m (3 3) ()) \
          (a-clef-1 (2 2) (array a-num-0 v-text-0 a-text-0 a-late a-text-1)) \
          (a-num-1 (2 2) ()) \
          (a-text-1 (3 3) ()) \
          (v-clef-free (18446744073709551615 0) (array a-text-5 a-text-6 v-text-free)) \
          (v-text-free (18446744073709551615 0) ()) \
          (a-bar (6 6) (array a-text-3 a-num-4 v-text-4)) \
          (a-time (6 6) (array a-text-3 a-num-4 v-text-4)) \
          (a-text-3 (7 7) ()) \
          (p-text-3 (7 7) ()) \
          (p-clef-3 (6 6) ()) \
          (v-text-4 (9 9) ()) \
          (v-key-late (2 2) (array a-num-0 v-text-0 a-text-0 a-late a-text-1)) \
          (a-num-4 (8 8) ()) \
          (a-key-5 (10 10) (array a-text-3 a-num-4 v-text-4 a-text-5 a-text-6 v-text-free)) \
          (a-text-5 (11 11) ()) \
          (a-text-6 (13 13) ()) \
          (a-late (1 1) ()))"
    );
}

/// Global with VerticalAxisGroup's and the columns' descriptions; a Score
/// with `Paper_column_engraver` and `Keep_probe`; a Group with
/// `Keep_alive_together_engraver`; a Staff with `Axis_group_engraver`.
const KEEP_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((VerticalAxisGroup (axes 1)
                           (meta (name . VerticalAxisGroup) (classes Spanner)
                                 (interfaces grob-interface axis-group-interface
                                             hara-kiri-group-spanner-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Group) (accepts Staff)
      (consists Paper_column_engraver) (consists Keep_probe))",
    "((context-name Group) (translator-type Engraver_group) (accepts Staff)
      (consists Keep_alive_together_engraver))",
    "((context-name Staff) (translator-type Engraver_group) (consists Axis_group_engraver))",
];

/// The probe's `records.scm` and its probe, written as its
/// `probe-keep-alive.ly` writes them, each record kept in `probed`.
const KEEP_PROBES: &str = "
    (set-object-property! 'hasAxisGroup 'translation-type? boolean?)
    (set-object-property! 'VerticalAxisGroup 'is-grob? #t)
    (define (key? x) (or (symbol? x) (and (exact-integer? x) (>= x 0))))
    (set-object-property! 'remove-layer 'backend-type? key?)
    ;; records.scm
    (define labels '())
    (define (gname g) (if (ly:grob? g) (assq-ref (ly:grob-property g 'meta) 'name) g))
    (define (label g)
      (cond ((not (ly:grob? g)) g)
            ((assq-ref labels g) (assq-ref labels g))
            (else (gname g))))
    (define-public probed '())
    (define (out x) (set! probed (cons x probed)))
    (define (arr x)
      (if (ly:grob-array? x) (cons 'array (map label (ly:grob-array->list x))) x))
    (define (kept-alive g)
      (list (label g) (ly:grob-property-data g 'remove-layer)
            (arr (ly:grob-object g 'keep-alive-with)) (arr (ly:grob-object g 'make-dead-when))))
    ;; probe-keep-alive.ly: each hara-kiri group acknowledged, labelled by
    ;; its engraver's context id; at the finalize, after the Groups', each
    ;; one's label, remove-layer, keep-alive-with and make-dead-when.
    (ly:register-translator
      (lambda (context)
        (let ((seen '()))
          `((acknowledgers
             (hara-kiri-group-spanner-interface
              . ,(lambda (t g s)
                   (set! labels
                         (acons g (list (gname g) (ly:context-id (ly:translator-context s)))
                                labels))
                   (set! seen (cons g seen)))))
            (finalize
             . ,(lambda (t) (for-each (lambda (g) (out (kept-alive g))) (reverse seen)))))))
      'Keep_probe)
    (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
    (define (s) (make-music 'SkipEvent 'duration (ly:make-duration 2)))
    (define (new type m)
      (make-music 'ContextSpeccedMusic 'context-type type 'create-new #t 'element m))
    (define (new-id type id m)
      (make-music 'ContextSpeccedMusic 'context-type type 'context-id id 'create-new #t
                  'element m))
    (define (layer value)
      (make-music 'ContextSpeccedMusic 'context-type 'Staff 'element
                  (make-music 'OverrideProperty 'symbol 'VerticalAxisGroup
                              'grob-property-path '(remove-layer) 'grob-value value
                              'pop-first #t)))
    ;; A Staff whose group has `remove-layer` value, or none with 'unset.
    (define (staff id value . skips)
      (new-id 'Staff id
              (apply seq (if (eq? value 'unset) skips (cons (layer value) skips)))))";

/// `Keep_alive_together_engraver` (MS3 step 21,
/// `keep-alive-together-engraver.cc`), layout's class found before the
/// music crate's placeholder: at its finalize, each hara-kiri group of its
/// context and below it, in acknowledgement order, takes the others by
/// its `remove-layer`: `any` all of them, a `#f` one too; `above` the one
/// acknowledged just before it ("late", written first, acknowledged last),
/// `below` the one just after it, none for the first or the last
/// ("lower"); an unknown symbol LilyPond's warning once for each other
/// group; unset, all but the `#f` ones; an integer, only the integers: an
/// equal one into its `keep-alive-with`, a lower one into its
/// `make-dead-when`. A `#f` layer takes none, an empty array is not set,
/// and a group in no such context ("solo") is not touched. As LilyPond
/// 2.26.0 gives the probe's score
/// (`$LILY_CORPUS/device-runs/2026-10-11-ms3-step21/session6-probe/`,
/// `probe-keep-alive.out`, and its warnings in `probe-keep-alive.log`).
#[test]
fn keep_alive_together_engraver_ties_the_groups_as_lilypond_does() {
    use lily_engine_layout::axis_group_engraver::AXIS_GROUP_ENGRAVER;
    use lily_engine_layout::keep_alive_together_engraver::KEEP_ALIVE_TOGETHER_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(KEEP_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, KEEP_PROBES);
    let music = eval(
        &mut vm,
        "(new 'Score (par
          (new-id 'Group \"upper\"
                  (par (seq (s) (staff \"late\" 'above (s) (s) (s)))
                       (staff \"unset\" 'unset (s) (s) (s) (s))
                       (staff \"one\" 1 (s) (s) (s) (s))
                       (staff \"two\" 2 (s) (s) (s) (s))
                       (staff \"one-b\" 1 (s) (s) (s) (s))
                       (staff \"false\" #f (s) (s) (s) (s))
                       (staff \"any\" 'any (s) (s) (s) (s))
                       (staff \"above\" 'above (s) (s) (s) (s))
                       (staff \"below\" 'below (s) (s) (s) (s))
                       (staff \"bad\" 'bogus (s) (s) (s) (s))
                       (staff \"zero\" 0 (s) (s) (s) (s))))
          (new-id 'Group \"lower\"
                  (par (staff \"above-2\" 'above (s) (s) (s) (s))
                       (staff \"three\" 3 (s) (s) (s) (s))
                       (staff \"below-2\" 'below (s) (s) (s) (s))))
          (staff \"solo\" 1 (s) (s) (s) (s))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &AXIS_GROUP_ENGRAVER,
        &KEEP_ALIVE_TOGETHER_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    let warning = "(warning \"unknown remove-layer value `bogus'\")";
    assert_eq!(
        show_of(&mut vm, "(kept)"),
        format!("({})", [warning; 10].join(" "))
    );
    let g = |id: &str| format!("(VerticalAxisGroup \"{id}\")");
    let array = |ids: &[&str]| {
        let labels: Vec<String> = ids.iter().map(|id| g(id)).collect();
        format!("(array {})", labels.join(" "))
    };
    let record = |id: &str, layer: &str, live: &str, dead: &str| {
        format!("({} {layer} {live} {dead})", g(id))
    };
    let records = [
        record(
            "unset",
            "()",
            &array(&[
                "one", "two", "one-b", "any", "above", "below", "bad", "zero", "late",
            ]),
            "()",
        ),
        record("one", "1", &array(&["one-b"]), &array(&["zero"])),
        record("two", "2", "()", &array(&["one", "one-b", "zero"])),
        record("one-b", "1", &array(&["one"]), &array(&["zero"])),
        record("false", "#f", "()", "()"),
        record(
            "any",
            "any",
            &array(&[
                "unset", "one", "two", "one-b", "false", "above", "below", "bad", "zero", "late",
            ]),
            "()",
        ),
        record("above", "above", &array(&["any"]), "()"),
        record("below", "below", &array(&["bad"]), "()"),
        record("bad", "bogus", "()", "()"),
        record("zero", "0", "()", "()"),
        record("above-2", "above", "()", "()"),
        record("three", "3", "()", "()"),
        record("below-2", "below", "()", "()"),
        record("solo", "1", "()", "()"),
        record("late", "above", &array(&["zero"]), "()"),
    ];
    assert_eq!(
        show_of(&mut vm, "(reverse probed)"),
        format!("({})", records.join(" "))
    );
}

/// Global with TextScript's, MultiMeasureRest's and the columns'
/// descriptions; a Score with `Paper_column_engraver` and `Pq_writer`; a
/// Staff with `Pq_echo`, `Grob_pq_engraver` and `Pq_probe` holding Voices
/// with `Pq_maker`, `Grob_pq_engraver` and `Pq_probe`, and a Plain with
/// `Pq_maker` alone.
const PQ_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((TextScript (meta (name . TextScript) (classes Item)
                          (interfaces grob-interface text-script-interface)))
        (MultiMeasureRest (meta (name . MultiMeasureRest) (classes Spanner)
                                (interfaces grob-interface multi-measure-interface
                                            multi-measure-rest-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (consists Paper_column_engraver) (consists Pq_writer))",
    "((context-name Staff) (translator-type Engraver_group) (accepts Voice) (accepts Plain)
      (consists Pq_echo) (consists Grob_pq_engraver) (consists Pq_probe))",
    "((context-name Voice) (translator-type Engraver_group) (consists Pq_maker)
      (consists Grob_pq_engraver) (consists Pq_probe))",
    "((context-name Plain) (translator-type Engraver_group) (consists Pq_maker))",
];

/// The probe's `records.scm`, its plan, its maker, echo and probes and
/// its music's helpers, written as its `probe-pq.ly` writes them, each
/// record kept in `probed`.
const PQ_PROBES: &str = "
    (set-object-property! 'busyGrobs 'translation-type? list?)
    (define-public LEFT -1)
    (define-public RIGHT 1)
    ;; records.scm
    (define labels '())
    (define (gname g) (if (ly:grob? g) (assq-ref (ly:grob-property g 'meta) 'name) g))
    (define (label g)
      (cond ((not (ly:grob? g)) g)
            ((assq-ref labels g) (assq-ref labels g))
            (else (gname g))))
    (define-public probed '())
    (define (out x) (set! probed (cons x probed)))
    (define (mom m) (list (ly:moment-main m) (ly:moment-grace m)))
    (define (busy l)
      (map (lambda (e) (append (mom (car e)) (list (label (cdr e))))) l))
    (define records '())
    (define (record! id kind now l) (set! records (cons (list id kind now l) records)))
    (define (records-of id) (filter (lambda (r) (equal? (car r) id)) (reverse records)))
    ;; probe-pq.ly
    (define plan
      '(((\"v1\" 0 0) (TextScript none none)
                      (TextScript by-grob (grob . n1))
                      (MultiMeasureRest mmr (event . n2) spanner))))
    (define made '())
    (define events '())
    (define (tag ev) (car (ly:event-property ev 'tags)))
    (define (make! engraver type t cause)
      (let ((g (ly:engraver-make-grob engraver type cause)))
        (set! labels (acons g t labels))
        (set! made (acons t g made))
        g))
    ;; A TextScript of each note heard, caused by it and labelled by its
    ;; tag, then the grobs of `plan` for the context's (id main grace).
    (ly:register-translator
      (lambda (context)
        (let ((heard '()))
          `((listeners
             (note-event
              . ,(lambda (engraver event)
                   (set! heard (cons event heard))
                   (set! events (acons (tag event) event events)))))
            (process-music
             . ,(lambda (engraver)
                  (for-each (lambda (ev) (make! engraver 'TextScript (tag ev) ev)) (reverse heard))
                  (set! heard '())
                  (for-each
                   (lambda (spec)
                     (let* ((c (caddr spec))
                            (g (make! engraver (car spec) (cadr spec)
                                      (cond ((eq? c 'none) '())
                                            ((eq? (car c) 'grob) (assq-ref made (cdr c)))
                                            (else (assq-ref events (cdr c)))))))
                       (if (memq 'spanner (cdddr spec))
                           (let ((col (ly:context-property context 'currentMusicalColumn)))
                             (ly:spanner-set-bound! g LEFT col)
                             (ly:spanner-set-bound! g RIGHT col)))))
                   (or (assoc-ref plan (cons (ly:context-id context)
                                             (mom (ly:context-current-moment context))))
                       '())))))))
      'Pq_maker)
    ;; In process-acknowledged after it saw n6's grob, a TextScript `echo`
    ;; with n6's cause.
    (ly:register-translator
      (lambda (context)
        (let ((wait #f))
          `((acknowledgers
             (grob-interface
              . ,(lambda (engraver grob source-engraver)
                   (if (eq? (label grob) 'n6) (set! wait grob)))))
            (process-acknowledged
             . ,(lambda (engraver)
                  (if wait
                      (begin
                       (make! engraver 'TextScript 'echo (ly:grob-property wait 'cause))
                       (set! wait #f))))))))
      'Pq_echo)
    ;; busyGrobs at each start-translation-timestep and, when it changed,
    ;; after each process-acknowledged; in u, busyGrobs unset at each
    ;; stop-translation-timestep and at each start only whether u holds it.
    (ly:register-translator
      (lambda (context)
        (let ((last #f))
          (define (note! kind)
            (let ((b (busy (ly:context-property context 'busyGrobs))))
              (if (or (eq? kind 'start) (not (equal? b last)))
                  (record! (ly:context-id context) kind
                           (mom (ly:context-current-moment context)) b))
              (set! last b)))
          (define (u?) (equal? (ly:context-id context) \"u\"))
          `((start-translation-timestep
             . ,(lambda (engraver)
                  (if (u?)
                      (record! \"u\" 'own (mom (ly:context-current-moment context))
                               (eq? (ly:context-property-where-defined context 'busyGrobs)
                                    context))
                      (note! 'start))))
            (process-acknowledged . ,(lambda (engraver) (if (not (u?)) (note! 'ack))))
            (stop-translation-timestep
             . ,(lambda (engraver)
                  (if (u?) (ly:context-unset-property context 'busyGrobs)))))))
      'Pq_probe)
    ;; At the finalize, the records of each context in turn.
    (ly:register-translator
      (lambda (context)
        `((finalize
           . ,(lambda (engraver)
                (for-each (lambda (id) (for-each out (records-of id)))
                          '(\"a\" \"v1\" \"v2\" \"v3\" \"u\"))))))
      'Pq_writer)
    (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
    (define (s) (make-music 'SkipEvent 'duration (ly:make-duration 2)))
    (define (new type m)
      (make-music 'ContextSpeccedMusic 'context-type type 'create-new #t 'element m))
    (define (new-id type id m)
      (make-music 'ContextSpeccedMusic 'context-type type 'context-id id 'create-new #t
                  'element m))
    ;; A note tagged t of duration log (scaled by scale, if given).
    (define (n t log . scale)
      (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0)
                  'duration (if (null? scale)
                                (ly:make-duration log)
                                (ly:make-duration log 0 (car scale)))
                  'tags (list t)))
    (define (chord . notes) (make-music 'EventChord 'elements notes))
    (define (grace m) (make-music 'GraceMusic 'element m))";

/// LilyPond 2.26.0's records of the probe's score, one a line, each
/// `(id kind (main grace) busyGrobs)` with busyGrobs' entries as `(main
/// grace label)`; u's `("u" own (main grace) <whether u holds busyGrobs>)`.
const PQ_RECORDS: &str = r#"
("a" ack (0 0) ())
("a" ack (0 0) ((1/4 0 n1) (1/4 0 w1) (1/2 0 n2) (1/2 0 n6)))
("a" ack (0 0) ((1/4 0 n1) (1/4 0 w1) (1/2 0 echo) (1/2 0 n2) (1/2 0 n6)))
("a" start (1/4 0) ((1/4 0 n1) (1/4 0 w1) (1/2 0 echo) (1/2 0 n2) (1/2 0 n6)))
("a" ack (1/4 0) ((1/4 0 n1) (1/4 0 w1) (1/2 0 echo) (1/2 0 n2) (1/2 0 n6) (3/4 0 n12)))
("a" start (1/2 0) ((1/2 0 echo) (1/2 0 n2) (1/2 0 n6) (3/4 0 n12)))
("a" ack (1/2 0) ((1/2 0 echo) (1/2 0 n2) (1/2 0 n6) (5/8 0 n7) (3/4 0 n3) (3/4 0 n12)))
("a" start (5/8 0) ((5/8 0 n7) (3/4 0 n3) (3/4 0 n12)))
("a" ack (5/8 0) ((5/8 0 n7) (3/4 0 n8) (3/4 0 n3) (3/4 0 n12)))
("a" start (3/4 -1/8) ((3/4 0 n8) (3/4 0 n3) (3/4 0 n12)))
("a" ack (3/4 -1/8) ((3/4 -1/16 g1) (3/4 0 n8) (3/4 0 n3) (3/4 0 n12)))
("a" start (3/4 -1/16) ((3/4 -1/16 g1) (3/4 0 n8) (3/4 0 n3) (3/4 0 n12)))
("a" ack (3/4 -1/16) ((3/4 -1/16 g1) (3/4 0 g2) (3/4 0 n8) (3/4 0 n3) (3/4 0 n12)))
("a" start (3/4 0) ((3/4 0 g2) (3/4 0 n8) (3/4 0 n3) (3/4 0 n12)))
("a" ack (3/4 0) ((3/4 0 g2) (3/4 0 n8) (3/4 0 n3) (3/4 0 n12) (1 0 n4) (1 0 n9)))
("a" start (1 0) ((1 0 n4) (1 0 n9)))
("a" ack (1 0) ((1 0 n4) (1 0 n9) (9/8 0 n5b) (5/4 0 n10) (3/2 0 n5)))
("a" start (5/4 0) ((5/4 0 n10) (3/2 0 n5)))
("a" ack (5/4 0) ((5/4 0 n10) (3/2 0 n11) (3/2 0 n5)))
("a" start (3/2 0) ((3/2 0 n11) (3/2 0 n5)))
("v1" ack (0 0) ())
("v1" ack (0 0) ((1/4 0 n1) (1/2 0 n2)))
("v1" start (1/4 0) ((1/4 0 n1) (1/2 0 n2)))
("v1" start (1/2 0) ((1/2 0 n2)))
("v1" ack (1/2 0) ((1/2 0 n2) (3/4 0 n3)))
("v1" start (5/8 0) ((3/4 0 n3)))
("v1" start (3/4 -1/8) ((3/4 0 n3)))
("v1" ack (3/4 -1/8) ((3/4 -1/16 g1) (3/4 0 n3)))
("v1" start (3/4 -1/16) ((3/4 -1/16 g1) (3/4 0 n3)))
("v1" ack (3/4 -1/16) ((3/4 -1/16 g1) (3/4 0 g2) (3/4 0 n3)))
("v1" start (3/4 0) ((3/4 0 g2) (3/4 0 n3)))
("v1" ack (3/4 0) ((3/4 0 g2) (3/4 0 n3) (1 0 n4)))
("v1" start (1 0) ((1 0 n4)))
("v1" ack (1 0) ((1 0 n4) (9/8 0 n5b) (3/2 0 n5)))
("v1" start (5/4 0) ((3/2 0 n5)))
("v1" start (3/2 0) ((3/2 0 n5)))
("v2" ack (0 0) ())
("v2" ack (0 0) ((1/2 0 n6)))
("v2" start (1/4 0) ((1/2 0 n6)))
("v2" start (1/2 0) ((1/2 0 n6)))
("v2" ack (1/2 0) ((1/2 0 n6) (5/8 0 n7)))
("v2" start (5/8 0) ((5/8 0 n7)))
("v2" ack (5/8 0) ((5/8 0 n7) (3/4 0 n8)))
("v2" start (3/4 -1/8) ((3/4 0 n8)))
("v2" start (3/4 -1/16) ((3/4 0 n8)))
("v2" start (3/4 0) ((3/4 0 n8)))
("v2" ack (3/4 0) ((3/4 0 n8) (1 0 n9)))
("v2" start (1 0) ((1 0 n9)))
("v2" ack (1 0) ((1 0 n9) (5/4 0 n10)))
("v2" start (5/4 0) ((5/4 0 n10)))
("v2" ack (5/4 0) ((5/4 0 n10) (3/2 0 n11)))
("v2" start (3/2 0) ((3/2 0 n11)))
("v3" ack (1/4 0) ())
("v3" ack (1/4 0) ((3/4 0 n12)))
("v3" start (1/2 0) ((3/4 0 n12)))
("v3" start (5/8 0) ((3/4 0 n12)))
("v3" start (3/4 -1/8) ((3/4 0 n12)))
("v3" start (3/4 -1/16) ((3/4 0 n12)))
("v3" start (3/4 0) ((3/4 0 n12)))
("v3" start (1 0) ())
("v3" start (5/4 0) ())
("v3" start (3/2 0) ())
("u" own (1/4 0) #f)
("u" own (1/2 0) #f)
("u" own (5/8 0) #f)
("u" own (3/4 -1/8) #f)
("u" own (3/4 -1/16) #f)
("u" own (3/4 0) #f)
("u" own (1 0) #f)
("u" own (5/4 0) #f)
("u" own (3/2 0) #f)
"#;

/// `Grob_pq_engraver` (MS3 step 21, `grob-pq-engraver.cc`), layout's
/// class found before the music crate's placeholder: `busyGrobs` in each
/// Staff and Voice, set to `()` at its initialize (v3's, made at 1/4 under
/// a's full list), takes each grob of its context and below it with an
/// event cause and an event length: the grobs of one timestep sorted by
/// their ends, equal ends in acknowledgement order, then merged before the
/// entries already there with equal ends, also in a second round (a's
/// echo); a grace's length in the grace part, the ends compared main part
/// first; none for a grob of no cause (`none`) or of a grob's (`by-grob`),
/// a multi-measure rest (`mmr`) or a zero length (`z`). At each timestep's
/// start the leading entries that ended before it are dropped, so an entry
/// ending now stays (n1, w1 at 1/4) and one ending with no timestep at its
/// end (n5b at 9/8) goes at the next (5/4). A context's first timestep
/// starts before it is made, so its first record is an `ack`. In u, of
/// skips alone, busyGrobs is unset at each stop: u's start reads a's list,
/// already pruned by a's start, drops nothing and so sets nothing (`own`
/// stays #f, `scm_is_eq`). As LilyPond
/// 2.26.0 gives the probe's score
/// (`$LILY_CORPUS/device-runs/2026-10-11-ms3-step21/session7-probe/`,
/// `probe-pq.out`).
#[test]
fn grob_pq_engraver_tracks_busy_grobs_as_lilypond_does() {
    use lily_engine_layout::grob_pq_engraver::GROB_PQ_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(PQ_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, PQ_PROBES);
    let music = eval(
        &mut vm,
        "(new 'Score
          (new-id 'Staff \"a\"
           (par (new-id 'Voice \"v1\"
                        (seq (chord (n 'n1 2) (n 'n2 1))
                             (chord (n 'n3 2) (n 'z 2 0))
                             (grace (seq (n 'g1 4) (n 'g2 4)))
                             (n 'n4 2)
                             (chord (n 'n5 1) (n 'n5b 3))))
                (new-id 'Voice \"v2\"
                        (seq (n 'n6 1) (n 'n7 3) (n 'n8 3) (n 'n9 2) (n 'n10 2) (n 'n11 2)))
                (seq (s) (new-id 'Voice \"v3\" (seq (n 'n12 1) (s) (s) (s))))
                (new-id 'Plain \"w\" (seq (n 'w1 2) (s) (s) (s) (s) (s)))
                (new-id 'Voice \"u\" (seq (s) (s) (s) (s) (s) (s))))))",
    );
    vm.heap_mut().root(music);
    let classes = [&SCORE_ENGRAVER, &PAPER_COLUMN_ENGRAVER, &GROB_PQ_ENGRAVER];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(show_of(&mut vm, "(kept)"), "()");
    let probed = eval(&mut vm, "(reverse probed)");
    let mut records = Vec::new();
    let mut p = probed;
    while let (Some(r), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
        records.push(show(&vm, r));
        p = rest;
    }
    let lilyponds: Vec<&str> = PQ_RECORDS.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lilyponds.len(), 71);
    assert_eq!(records, lilyponds);
}

/// Global with NoteHead's, TextScript's and the columns' descriptions; a
/// Score with `Fb_probe` and `Paper_column_engraver`, holding Voices with
/// `Fb_maker`, `Grob_pq_engraver` and `Forbid_line_break_engraver` and a
/// Plain with `Fb_maker` and `Grob_pq_engraver`.
const FB_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface rhythmic-grob-interface)))
        (TextScript (meta (name . TextScript) (classes Item)
                          (interfaces grob-interface text-script-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice) (accepts Plain)
      (consists Fb_probe) (consists Paper_column_engraver))",
    "((context-name Voice) (translator-type Engraver_group) (consists Fb_maker)
      (consists Grob_pq_engraver) (consists Forbid_line_break_engraver))",
    "((context-name Plain) (translator-type Engraver_group) (consists Fb_maker)
      (consists Grob_pq_engraver))",
];

/// The probe's maker and probe and its music's helpers, written as its
/// `probe-fb.ly` writes them, each record kept in `probed`.
const FB_PROBES: &str = "
    (set-object-property! 'busyGrobs 'translation-type? list?)
    (set-object-property! 'forbidBreak 'translation-type? boolean?)
    (define-public probed '())
    (define (out x) (set! probed (cons x probed)))
    (define (mom m) (list (ly:moment-main m) (ly:moment-grace m)))
    (define (tag ev) (car (ly:event-property ev 'tags)))
    ;; A grob of each note heard, caused by it: a TextScript for t1, else a
    ;; NoteHead.
    (ly:register-translator
      (lambda (context)
        (let ((heard '()))
          `((listeners
             (note-event . ,(lambda (engraver event) (set! heard (cons event heard)))))
            (process-music
             . ,(lambda (engraver)
                  (for-each (lambda (ev)
                              (ly:engraver-make-grob
                               engraver (if (eq? (tag ev) 't1) 'TextScript 'NoteHead) ev))
                            (reverse heard))
                  (set! heard '()))))))
      'Fb_maker)
    ;; Now and the Score's forbidBreak at each process-music.
    (ly:register-translator
      (lambda (context)
        `((process-music
           . ,(lambda (engraver)
                (out (list (mom (ly:context-current-moment context))
                           (ly:context-property context 'forbidBreak)))))))
      'Fb_probe)
    (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
    (define (s) (make-music 'SkipEvent 'duration (ly:make-duration 2)))
    (define (new type m)
      (make-music 'ContextSpeccedMusic 'context-type type 'create-new #t 'element m))
    (define (new-id type id m)
      (make-music 'ContextSpeccedMusic 'context-type type 'context-id id 'create-new #t
                  'element m))
    ;; A note tagged t of duration log.
    (define (n t log)
      (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration log)
                  'tags (list t)))
    (define (chord . notes) (make-music 'EventChord 'elements notes))
    (define (grace m) (make-music 'GraceMusic 'element m))";

/// LilyPond 2.26.0's records of the probe's score, one a line, each `((main
/// grace) forbidBreak)`.
const FB_RECORDS: &str = "
((0 0) ())
((1/4 0) #t)
((1/2 0) ())
((3/4 -1/16) ())
((3/4 0) ())
((1 0) #t)
((5/4 0) ())
((3/2 0) ())
";

/// `Forbid_line_break_engraver` (MS3 step 21, `forbid-break-engraver.cc`),
/// layout's class found before the music crate's placeholder: at each
/// `pre_process_music` the Score's `forbidBreak` is set while a grob with
/// `rhythmic-grob-interface` in its Voice's `busyGrobs` plays on past now
/// (v1's n1 at 1/4, n3b at 1 after n3, which ends then and is passed
/// over), and not for an entry whose end's main part is now's, also at a
/// grace timestep (n2 at 3/4 -1/16), nor for a TextScript (v2's t1), nor
/// for the busy grobs of a context without the engraver (w's w1);
/// `Paper_column_engraver` unsets it at each stop. As LilyPond 2.26.0
/// gives the probe's score
/// (`$LILY_CORPUS/device-runs/2026-10-11-ms3-step21/session8-probe/`,
/// `probe-fb.out`).
#[test]
fn forbid_line_break_engraver_forbids_breaks_as_lilypond_does() {
    use lily_engine_layout::forbid_line_break_engraver::FORBID_LINE_BREAK_ENGRAVER;
    use lily_engine_layout::grob_pq_engraver::GROB_PQ_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(FB_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, FB_PROBES);
    let music = eval(
        &mut vm,
        "(new 'Score
          (par (new-id 'Voice \"v1\"
                       (seq (n 'n1 1) (n 'n2 2) (grace (n 'g1 4))
                            (chord (n 'n3 2) (n 'n3b 1)) (n 'n4 2)))
               (new-id 'Voice \"v2\" (seq (s) (n 't1 0)))
               (new-id 'Plain \"w\" (seq (n 'w1 0) (s)))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &GROB_PQ_ENGRAVER,
        &FORBID_LINE_BREAK_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(show_of(&mut vm, "(kept)"), "()");
    let probed = eval(&mut vm, "(reverse probed)");
    let mut records = Vec::new();
    let mut p = probed;
    while let (Some(r), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
        records.push(show(&vm, r));
        p = rest;
    }
    let lilyponds: Vec<&str> = FB_RECORDS.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lilyponds.len(), 8);
    assert_eq!(records, lilyponds);
}

/// Global with TextScript's description (its `side-axis` and
/// `script-priority` LilyPond's) and the columns'; a Score with
/// `Paper_column_engraver`, holding a Voice with `Y_maker`.
const Y_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((TextScript (script-priority . 200) (side-axis . 1)
                    (meta (name . TextScript) (classes Item)
                          (interfaces grob-interface side-position-interface text-interface
                                      text-script-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice)
      (consists Paper_column_engraver))",
    "((context-name Voice) (translator-type Engraver_group) (consists Y_maker))",
];

/// The probe's maker and its music's helpers, written as its `probe-y.ly`
/// writes them, `point-stencil` as `stencil.scm` makes it; each grob made
/// kept in `y-made` with its label.
const Y_PROBES: &str = "
    (set-object-property! 'side-axis 'backend-type? number?)
    (set-object-property! 'stencil 'backend-type? ly:stencil?)
    (define-public y-made '())
    (define (tags ev) (ly:event-property ev 'tags))
    (define stencil (ly:make-stencil \"\" '(0 . 0) '(0 . 0)))
    (ly:register-translator
      (lambda (context)
        (let ((heard '()))
          `((listeners
             (note-event . ,(lambda (engraver event) (set! heard (cons event heard)))))
            (process-music
             . ,(lambda (engraver)
                  (for-each
                   (lambda (ev)
                     (let* ((t (tags ev))
                            (g (ly:engraver-make-grob engraver 'TextScript ev)))
                       (set! y-made (append y-made (list (cons (car t) g))))
                       (if (not (eq? (cadr t) 'keep)) (ly:grob-set-property! g 'side-axis (cadr t)))
                       (ly:grob-set-property! g 'stencil (if (memq 'none t) #f stencil))))
                   (reverse heard))
                  (set! heard '()))))))
      'Y_maker)
    (define (new type m)
      (make-music 'ContextSpeccedMusic 'context-type type 'create-new #t 'element m))
    (define (new-id type id m)
      (make-music 'ContextSpeccedMusic 'context-type type 'context-id id 'create-new #t
                  'element m))
    (define (n . t)
      (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration 2)
                  'tags t))
    (define (chord . notes) (make-music 'EventChord 'elements notes))";

/// `Side_position_interface::is_on_y_axis` (MS3 step 22, part 4's, ported
/// first for `Non_musical_script_column_engraver`): whether a grob's
/// `side-axis` is Y (1), with LilyPond's programming error for one on no
/// axis (7, -1) that has a stencil, and none for one without. As LilyPond
/// 2.26.0's `Script_column_engraver` (`script-column-engraver.cc:58`)
/// takes the probe's TextScripts into its ScriptColumn, all of them
/// musical with a `script-priority`, and its two programming errors
/// (`$LILY_CORPUS/device-runs/2026-10-13-ms3-step22/session1-probe/`,
/// `probe-y.out`, `probe-y.log`).
#[test]
fn is_on_y_axis_takes_the_side_axis_as_lilypond_does() {
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::side_position::is_on_y_axis;
    let Some((mut vm, layout)) = vm_with(Y_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, Y_PROBES);
    let music = eval(
        &mut vm,
        "(new 'Score
          (new-id 'Voice \"v\"
           (chord (n 'y1 1) (n 'x0 0) (n 's7 7) (n 'm1 -1) (n 'n7 7 'none) (n 'y1n 1 'none)
                  (n 'keep 'keep))))",
    );
    vm.heap_mut().root(music);
    let classes = [&SCORE_ENGRAVER, &PAPER_COLUMN_ENGRAVER];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(show_of(&mut vm, "(kept)"), "()");
    let made = eval(&mut vm, "y-made");
    let mut on_y = Vec::new();
    let mut p = made;
    while let (Some(entry), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
        let (label, g) = (vm.heap().car(entry).unwrap(), vm.heap().cdr(entry).unwrap());
        if is_on_y_axis(&mut vm, g).unwrap() {
            on_y.push(show(&vm, label));
        }
        p = rest;
    }
    assert_eq!(on_y, ["y1", "y1n", "keep"]);
    let error = "(programming-error \"no side-axis setting found for grob TextScript.\")";
    assert_eq!(show_of(&mut vm, "(kept)"), format!("({error} {error})"));
}

/// Global with the descriptions of the grobs the maker makes (Clef's and
/// KeySignature's `non-musical` and `break-align-symbol` LilyPond's),
/// StaffSpacing's (its `non-musical` LilyPond's) and the columns'; a Score
/// with `Paper_column_engraver` and `Slg_writer`, holding a Staff with
/// `createSpacing` `#t`, `Separating_line_group_engraver` and `Slg_probe`,
/// whose Voices have `Slg_maker`, and a Plain with all three.
const SLG_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((Clef (break-align-symbol . clef) (non-musical . #t)
              (meta (name . Clef) (classes Item)
                    (interfaces grob-interface break-aligned-interface clef-interface)))
        (KeySignature (break-align-symbol . key-signature) (non-musical . #t)
                      (meta (name . KeySignature) (classes Item)
                            (interfaces grob-interface break-aligned-interface
                                        key-signature-interface)))
        (TextScript (meta (name . TextScript) (classes Item)
                          (interfaces grob-interface text-script-interface)))
        (NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface)))
        (NoteSpacing (meta (name . NoteSpacing) (classes Item)
                           (interfaces grob-interface note-spacing-interface spacing-interface)))
        (StaffSpacing (non-musical . #t)
                      (meta (name . StaffSpacing) (classes Item)
                            (interfaces grob-interface spacing-interface
                                        staff-spacing-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff) (accepts Plain)
      (consists Paper_column_engraver) (consists Slg_writer))",
    "((context-name Staff) (translator-type Engraver_group) (accepts Voice)
      (assign createSpacing #t) (consists Separating_line_group_engraver) (consists Slg_probe))",
    "((context-name Voice) (translator-type Engraver_group) (consists Slg_maker))",
    "((context-name Plain) (translator-type Engraver_group) (consists Slg_maker)
      (consists Separating_line_group_engraver) (consists Slg_probe))",
];

/// The probe's maker, probe and writer and its music's helpers, written as
/// its `probe-slg.ly` writes them, each record kept in `probed`.
const SLG_PROBES: &str = "
    (set-object-property! 'createSpacing 'translation-type? boolean?)
    (set-object-property! 'hasStaffSpacing 'translation-type? boolean?)
    (define-public X 0)
    (define-public probed '())
    (define (out x) (set! probed (cons x probed)))
    (define (mom m) (list (ly:moment-main m) (ly:moment-grace m)))
    (define labels '())
    (define made '())
    (define (made! g l) (set! labels (acons g l labels)) (set! made (append made (list g))))
    (define (gname g) (assq-ref (ly:grob-property g 'meta) 'name))
    (define (label g)
      (cond ((not (ly:grob? g)) g)
            ((assq-ref labels g) (assq-ref labels g))
            ((memq (gname g) '(PaperColumn NonMusicalPaperColumn))
             (list (gname g) (mom (ly:grob-property g 'when))))
            (else (gname g))))
    (define (arr x) (if (ly:grob-array? x) (map label (ly:grob-array->list x)) x))
    (define (record g)
      (list (label g) (null? (ly:grob-property g 'cause)) (label (ly:grob-parent g X))
            (arr (ly:grob-object g 'left-items)) (arr (ly:grob-object g 'right-items))
            (arr (ly:grob-object g 'left-break-aligned))
            (arr (ly:grob-object g 'right-break-aligned))))
    (define (tags ev) (ly:event-property ev 'tags))
    ;; A grob of each note heard, in the order heard, caused by it, named by
    ;; its second tag and labelled by its first; a Cmd is a TextScript and
    ;; an NsCmd a NoteSpacing on the command column.
    (define (grob-name name) (case name ((Cmd) 'TextScript) ((NsCmd) 'NoteSpacing) (else name)))
    (ly:register-translator
      (lambda (context)
        (let ((heard '()))
          `((listeners
             (note-event . ,(lambda (engraver event) (set! heard (cons event heard)))))
            (process-music
             . ,(lambda (engraver)
                  (for-each
                   (lambda (ev)
                     (let* ((name (cadr (tags ev)))
                            (g (ly:engraver-make-grob engraver (grob-name name) ev)))
                       (if (memq name '(Cmd NsCmd))
                           (ly:grob-set-parent!
                            g X (ly:context-property context 'currentCommandColumn)))
                       (made! g (car (tags ev)))))
                   (reverse heard))
                  (set! heard '()))))))
      'Slg_maker)
    (define (has id context)
      (out (list id (mom (ly:context-current-moment context))
                 (ly:context-property context 'hasStaffSpacing))))
    ;; Each StaffSpacing labelled; hasStaffSpacing at each process-music
    ;; and stop.
    (ly:register-translator
      (lambda (context)
        (let ((id (ly:context-id context)))
          `((acknowledgers
             (staff-spacing-interface
              . ,(lambda (t g s)
                   (made! g (list 'ss id (mom (ly:context-current-moment context)))))))
            (process-music . ,(lambda (t) (has (list id 'process) context)))
            (stop-translation-timestep . ,(lambda (t) (has (list id 'stop) context))))))
      'Slg_probe)
    ;; The Score's hasStaffSpacing at each stop; each labelled grob's
    ;; record at the finalize.
    (ly:register-translator
      (lambda (context)
        `((stop-translation-timestep . ,(lambda (t) (has '(score stop) context)))
          (finalize . ,(lambda (t) (for-each (lambda (g) (out (record g))) made)))))
      'Slg_writer)
    (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
    (define (s) (make-music 'SkipEvent 'duration (ly:make-duration 2)))
    (define (new type m)
      (make-music 'ContextSpeccedMusic 'context-type type 'create-new #t 'element m))
    (define (new-id type id m)
      (make-music 'ContextSpeccedMusic 'context-type type 'context-id id 'create-new #t
                  'element m))
    ;; A quarter note labelled l whose grob is name.
    (define (n l name)
      (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration 2)
                  'tags (list l name)))
    (define (chord . notes) (make-music 'EventChord 'elements notes))";

/// LilyPond 2.26.0's records of the probe's score, one a line.
const SLG_RECORDS: &str = r#"
(("a" process) (0 0) ())
(("p" process) (0 0) ())
(("a" stop) (0 0) #t)
(("p" stop) (0 0) ())
((score stop) (0 0) ())
(("a" process) (1/4 0) ())
(("p" process) (1/4 0) ())
(("a" stop) (1/4 0) ())
(("p" stop) (1/4 0) ())
((score stop) (1/4 0) ())
(("a" process) (1/2 0) ())
(("p" process) (1/2 0) ())
(("a" stop) (1/2 0) #t)
(("p" stop) (1/2 0) ())
((score stop) (1/2 0) ())
(("a" process) (3/4 0) ())
(("p" process) (3/4 0) ())
(("a" stop) (3/4 0) #t)
(("p" stop) (3/4 0) ())
((score stop) (3/4 0) ())
(("a" process) (1 0) ())
(("p" process) (1 0) ())
(("a" stop) (1 0) #t)
(("p" stop) (1 0) ())
((score stop) (1 0) ())
(("a" process) (5/4 0) ())
(("p" process) (5/4 0) ())
(("a" stop) (5/4 0) ())
(("p" stop) (5/4 0) ())
((score stop) (5/4 0) ())
(("a" process) (3/2 0) ())
(("p" process) (3/2 0) ())
(("a" stop) (3/2 0) #t)
(("p" stop) (3/2 0) ())
((score stop) (3/2 0) ())
(("a" process) (7/4 0) ())
(("p" process) (7/4 0) ())
(("a" stop) (7/4 0) ())
(("p" stop) (7/4 0) ())
((score stop) (7/4 0) ())
(("a" process) (2 0) ())
(("a" stop) (2 0) ())
((score stop) (2 0) ())
(c1 #f (NonMusicalPaperColumn (0 0)) () () () ())
(h0 #f (PaperColumn (0 0)) () () () ())
(n0 #f (PaperColumn (0 0)) () () () ())
(pn0 #f (PaperColumn (0 0)) () () () (pc1 pc2))
(pc0 #f (NonMusicalPaperColumn (0 0)) () () () ())
((ss "a" (0 0)) #t (NonMusicalPaperColumn (0 0)) ((NonMusicalPaperColumn (0 0))) ((PaperColumn (0 0))) (c1) ())
(n1 #f (PaperColumn (1/4 0)) () () () (c2 k2))
(pc1 #f (NonMusicalPaperColumn (1/4 0)) () () () ())
(c2 #f (NonMusicalPaperColumn (1/2 0)) () () () ())
(k2 #f (NonMusicalPaperColumn (1/2 0)) () () () ())
(x2 #f (NonMusicalPaperColumn (1/2 0)) () () () ())
(pc2 #f (NonMusicalPaperColumn (1/2 0)) () () () ())
((ss "a" (1/2 0)) #t (NonMusicalPaperColumn (1/2 0)) ((NonMusicalPaperColumn (1/2 0))) ((NonMusicalPaperColumn (3/4 0))) (c2 k2) ())
(x3 #f (NonMusicalPaperColumn (3/4 0)) () () () ())
((ss "a" (3/4 0)) #t (NonMusicalPaperColumn (3/4 0)) ((NonMusicalPaperColumn (3/4 0))) ((NonMusicalPaperColumn (1 0))) () ())
(c4 #f (NonMusicalPaperColumn (1 0)) () () () ())
(n4 #f (PaperColumn (1 0)) () () () (c6))
((ss "a" (1 0)) #t (NonMusicalPaperColumn (1 0)) ((NonMusicalPaperColumn (1 0))) ((PaperColumn (1 0))) (c4) ())
(h5 #f (PaperColumn (5/4 0)) () () () ())
(c6 #f (NonMusicalPaperColumn (3/2 0)) () () () ())
(h6 #f (PaperColumn (3/2 0)) () () () ())
((ss "a" (3/2 0)) #t (NonMusicalPaperColumn (3/2 0)) ((NonMusicalPaperColumn (3/2 0))) ((PaperColumn (3/2 0))) (c6) ())
(nx #f (NonMusicalPaperColumn (7/4 0)) () () () ())
"#;

/// `Separating_line_group_engraver` (MS3 step 22,
/// `separating-line-group-engraver.cc`), layout's class found before the
/// music crate's placeholder: in Staff "a" (`createSpacing` `#t`), a
/// StaffSpacing at each timestep with a non-musical item (a Clef, a
/// KeySignature, a TextScript on the command column; not at 1/4, with a
/// note spacing only, at 5/4, with a note head only, nor at 7/4, with a
/// note spacing on the command column), with no cause, on the
/// command column, `hasStaffSpacing` `#t` until the next timestep's start;
/// its `left-items` the command column, its `right-items` the musical
/// column, or the next staff spacing's command column alone where the
/// timestep after it had no note spacings (1/2, 3/4); its
/// `left-break-aligned` the timestep's break-aligned items; each note
/// spacing's `right-break-aligned` those of the next timestep with
/// spacings (n1, n4), or of each timestep until then in Plain "p", where
/// `createSpacing` is unset and no StaffSpacing is made (pn0). As LilyPond
/// 2.26.0 gives the probe's score
/// (`$LILY_CORPUS/device-runs/2026-10-13-ms3-step22/session1-probe/`,
/// `probe-slg.out`).
#[test]
fn separating_line_group_engraver_makes_staff_spacings_as_lilypond_does() {
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::separating_line_group_engraver::SEPARATING_LINE_GROUP_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(SLG_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, SLG_PROBES);
    let music = eval(
        &mut vm,
        "(new 'Score
          (par (new-id 'Staff \"a\"
                (par (new-id 'Voice \"c\"
                      (seq (chord (n 'c1 'Clef)) (s)
                           (chord (n 'c2 'Clef) (n 'k2 'KeySignature) (n 'x2 'Cmd))
                           (chord (n 'x3 'Cmd)) (chord (n 'c4 'Clef)) (s) (chord (n 'c6 'Clef))))
                     (new-id 'Voice \"v\"
                      (seq (chord (n 'h0 'NoteHead) (n 'n0 'NoteSpacing))
                           (chord (n 'n1 'NoteSpacing)) (s) (s) (chord (n 'n4 'NoteSpacing))
                           (chord (n 'h5 'NoteHead)) (chord (n 'h6 'NoteHead))
                           (chord (n 'nx 'NsCmd))))))
               (new-id 'Plain \"p\"
                (seq (chord (n 'pn0 'NoteSpacing) (n 'pc0 'Clef)) (chord (n 'pc1 'Clef))
                     (chord (n 'pc2 'Clef)) (s) (s) (s) (s)))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &SEPARATING_LINE_GROUP_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(show_of(&mut vm, "(kept)"), "()");
    let probed = eval(&mut vm, "(reverse probed)");
    let mut records = Vec::new();
    let mut p = probed;
    while let (Some(r), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
        records.push(show(&vm, r));
        p = rest;
    }
    let lilyponds: Vec<&str> = SLG_RECORDS.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lilyponds.len(), 66);
    assert_eq!(records, lilyponds);
}

/// Global with Beam's, Glissando's and TextSpanner's descriptions (the
/// first two with `unbreakable-spanner-interface`) and the columns'; a
/// Score with `Sbf_probe` and `Paper_column_engraver`, holding Voices with
/// `Sbf_maker` and `Spanner_break_forbid_engraver` and a Plain with
/// `Sbf_maker`.
const SBF_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((Beam (meta (name . Beam) (classes Spanner)
                    (interfaces grob-interface spanner-interface
                                unbreakable-spanner-interface)))
        (Glissando (meta (name . Glissando) (classes Spanner)
                         (interfaces grob-interface spanner-interface
                                     unbreakable-spanner-interface)))
        (TextSpanner (meta (name . TextSpanner) (classes Spanner)
                           (interfaces grob-interface spanner-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice) (accepts Plain)
      (consists Sbf_probe) (consists Paper_column_engraver))",
    "((context-name Voice) (translator-type Engraver_group) (consists Sbf_maker)
      (consists Spanner_break_forbid_engraver))",
    "((context-name Plain) (translator-type Engraver_group) (consists Sbf_maker))",
];

/// The probe's maker and probe and its music's helpers, written as its
/// `probe-sbf.ly` writes them, each record kept in `probed`.
const SBF_PROBES: &str = "
    (set-object-property! 'forbidBreak 'translation-type? boolean?)
    (set-object-property! 'breakable 'backend-type? boolean?)
    (define-public LEFT -1)
    (define-public RIGHT 1)
    (define-public probed '())
    (define (out x) (set! probed (cons x probed)))
    (define (mom m) (list (ly:moment-main m) (ly:moment-grace m)))
    (define spanners '())
    (define (tags ev) (ly:event-property ev 'tags))
    ;; Each note heard, by its tags (label kind [target]): none makes
    ;; nothing; end ends the target's spanner, its right bound the musical
    ;; column; breakable makes a Beam whose breakable is #t; else a spanner
    ;; of the kind, its left bound the musical column.
    (ly:register-translator
      (lambda (context)
        (let ((heard '()))
          `((listeners
             (note-event . ,(lambda (engraver event) (set! heard (cons event heard)))))
            (process-music
             . ,(lambda (engraver)
                  (let ((col (ly:context-property context 'currentMusicalColumn)))
                    (for-each
                     (lambda (ev)
                       (let ((l (car (tags ev))) (kind (cadr (tags ev))))
                         (case kind
                           ((none) #f)
                           ((end)
                            (let ((sp (assq-ref spanners (caddr (tags ev)))))
                              (ly:spanner-set-bound! sp RIGHT col)
                              (ly:engraver-announce-end-grob engraver sp ev)))
                           (else
                            (let ((sp (ly:engraver-make-grob
                                       engraver (if (eq? kind 'breakable) 'Beam kind) ev)))
                              (if (eq? kind 'breakable)
                                  (ly:grob-set-property! sp 'breakable #t))
                              (ly:spanner-set-bound! sp LEFT col)
                              (set! spanners (acons l sp spanners)))))))
                     (reverse heard)))
                  (set! heard '()))))))
      'Sbf_maker)
    ;; Now and the Score's forbidBreak at each process-music.
    (ly:register-translator
      (lambda (context)
        `((process-music
           . ,(lambda (engraver)
                (out (list (mom (ly:context-current-moment context))
                           (ly:context-property context 'forbidBreak)))))))
      'Sbf_probe)
    (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
    (define (s) (make-music 'SkipEvent 'duration (ly:make-duration 2)))
    (define (new type m)
      (make-music 'ContextSpeccedMusic 'context-type type 'create-new #t 'element m))
    (define (new-id type id m)
      (make-music 'ContextSpeccedMusic 'context-type type 'context-id id 'create-new #t
                  'element m))
    ;; A note of duration log tagged with tags.
    (define (n log . tags)
      (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration log)
                  'tags tags))
    (define (chord . notes) (make-music 'EventChord 'elements notes))
    (define (grace m) (make-music 'GraceMusic 'element m))";

/// LilyPond 2.26.0's records of the probe's score, one a line, each `((main
/// grace) forbidBreak)`.
const SBF_RECORDS: &str = "
((0 0) ())
((1/4 0) #t)
((1/2 0) #t)
((3/4 0) #t)
((1 0) ())
((5/4 0) ())
((3/2 0) ())
((7/4 -1/16) ())
((7/4 0) #t)
((2 0) #t)
((9/4 0) #t)
";

/// `Spanner_break_forbid_engraver` (MS3 step 22,
/// `spanner-break-forbid-engraver.cc`), layout's class found before the
/// music crate's placeholder: the Score's `forbidBreak` set at each
/// `pre_process_music` while a spanner with `unbreakable-spanner-interface`
/// acknowledged runs (v1's b1 from 0 to its end at 3/4, through timesteps
/// where nothing wakes the engraver; b2's end at 1/4 drops b2 alone, the
/// end of kb, never running, none; the grace z1, made at 7/4 -1/16, to the
/// music's end), and not for one whose `breakable` is `#t` (kb, v2's k1),
/// nor for a spanner without the interface
/// (t1), nor for one of a context without the engraver (w's w1);
/// `Paper_column_engraver` unsets it at each stop. As LilyPond 2.26.0
/// gives the probe's score
/// (`$LILY_CORPUS/device-runs/2026-10-13-ms3-step22/session2-probe/`,
/// `probe-sbf.out`).
#[test]
fn spanner_break_forbid_engraver_forbids_breaks_as_lilypond_does() {
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::spanner_break_forbid_engraver::SPANNER_BREAK_FORBID_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(SBF_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, SBF_PROBES);
    let music = eval(
        &mut vm,
        "(new 'Score
          (par (new-id 'Voice \"v1\"
                       (seq (chord (n 2 'b1 'Beam) (n 2 'b2 'Glissando) (n 2 'kb 'breakable))
                            (chord (n 2 'e2 'end 'b2)) (chord (n 2 'ekb 'end 'kb))
                            (chord (n 2 'e1 'end 'b1))
                            (n 2 'x 'none) (n 2 'x 'none) (n 2 'x 'none)
                            (grace (n 4 'z1 'Beam)) (n 2 'x 'none) (n 2 'x 'none)))
               (new-id 'Voice \"v2\"
                       (seq (s) (s) (s) (s)
                            (chord (n 2 'k1 'breakable) (n 2 't1 'TextSpanner)) (s)
                            (chord (n 2 'ek 'end 'k1) (n 2 'et 'end 't1)) (s)))
               (new-id 'Plain \"w\"
                       (seq (s) (s) (s) (s) (s) (s) (n 2 'w1 'Beam) (n 2 'ew 'end 'w1)))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &SPANNER_BREAK_FORBID_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(show_of(&mut vm, "(kept)"), "()");
    let probed = eval(&mut vm, "(reverse probed)");
    let mut records = Vec::new();
    let mut p = probed;
    while let (Some(r), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
        records.push(show(&vm, r));
        p = rest;
    }
    let lilyponds: Vec<&str> = SBF_RECORDS.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lilyponds.len(), 11);
    assert_eq!(records, lilyponds);
}

/// Global with the descriptions of the grobs the maker makes (their
/// interfaces LilyPond's; MmHead a NoteHead with `multi-measure-interface`),
/// SpacingSpanner's and the columns'; a Score with `Paper_column_engraver`,
/// `Spacing_engraver` and `Sp_writer`, holding Voices with `Sp_maker`.
const SP_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface rhythmic-grob-interface
                                    rhythmic-head-interface)))
        (MmHead (meta (name . MmHead) (classes Item)
                      (interfaces grob-interface multi-measure-interface note-head-interface
                                  rhythmic-grob-interface rhythmic-head-interface)))
        (Rest (meta (name . Rest) (classes Item)
                    (interfaces grob-interface rest-interface rhythmic-grob-interface
                                rhythmic-head-interface)))
        (LyricText (meta (name . LyricText) (classes Item)
                         (interfaces grob-interface lyric-syllable-interface
                                     rhythmic-grob-interface)))
        (NoteSpacing (meta (name . NoteSpacing) (classes Item)
                           (interfaces grob-interface note-spacing-interface spacing-interface)))
        (StaffSpacing (non-musical . #t)
                      (meta (name . StaffSpacing) (classes Item)
                            (interfaces grob-interface spacing-interface
                                        staff-spacing-interface)))
        (SpacingSpanner (meta (name . SpacingSpanner) (classes Spanner)
                              (interfaces grob-interface spanner-interface
                                          spacing-options-interface spacing-spanner-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice)
      (consists Paper_column_engraver) (consists Spacing_engraver) (consists Sp_writer))",
    "((context-name Voice) (translator-type Engraver_group) (consists Sp_maker))",
];

/// The probe's maker and writer and its music's helpers, written as its
/// `probe-sp.ly` writes them, each record kept in `probed`.
const SP_PROBES: &str = "
    (set-object-property! 'proportionalNotationDuration 'translation-type? rational?)
    (set-object-property! 'duration-log 'backend-type? integer?)
    (define-public LEFT -1)
    (define-public RIGHT 1)
    (define-public probed '())
    (define (out x) (set! probed (cons x probed)))
    (define (mom m) (list (ly:moment-main m) (ly:moment-grace m)))
    (define labels '())
    (define (made! g l) (set! labels (acons g l labels)))
    (define spanners '())
    (define columns '())
    (define (gname g) (assq-ref (ly:grob-property g 'meta) 'name))
    (define (label g)
      (cond ((not (ly:grob? g)) g)
            ((assq-ref labels g) (assq-ref labels g))
            ((memq (gname g) '(PaperColumn NonMusicalPaperColumn))
             (list (gname g) (mom (ly:grob-property g 'when))))
            (else (gname g))))
    (define (arr x) (if (ly:grob-array? x) (map label (ly:grob-array->list x)) x))
    (define (column c)
      (list (label c) (label (ly:grob-object c 'spacing))
            (ly:grob-property-data c 'shortest-playing-duration)
            (ly:grob-property-data c 'shortest-starter-duration)
            (ly:grob-property-data c 'used)))
    (define (spanner sp)
      (list (label sp) (null? (ly:grob-property sp 'cause))
            (label (ly:spanner-bound sp LEFT)) (label (ly:spanner-bound sp RIGHT))
            (arr (ly:grob-object sp 'wishes))))
    (define (tags ev) (ly:event-property ev 'tags))
    ;; A grob of each note and text script heard, in the order heard, named
    ;; by its second tag, labelled by its first, caused by it unless tagged
    ;; nocause, its duration-log 2.
    (ly:register-translator
      (lambda (context)
        (let* ((heard '())
               (hear (lambda (engraver event) (set! heard (cons event heard)))))
          `((listeners (note-event . ,hear) (text-script-event . ,hear))
            (process-music
             . ,(lambda (engraver)
                  (for-each
                   (lambda (ev)
                     (let* ((t (tags ev))
                            (g (ly:engraver-make-grob engraver (cadr t)
                                                      (if (memq 'nocause t) '() ev))))
                       (ly:grob-set-property! g 'duration-log 2)
                       (made! g (car t))))
                   (reverse heard))
                  (set! heard '()))))))
      'Sp_maker)
    ;; Each SpacingSpanner labelled; both columns kept at each stop; at the
    ;; finalize, each column's record, then each spanner's.
    (ly:register-translator
      (lambda (context)
        `((acknowledgers
           (spacing-spanner-interface
            . ,(lambda (t g s)
                 (set! spanners (append spanners (list g)))
                 (made! g (string->symbol (format #f \"sp~a\" (length spanners)))))))
          (stop-translation-timestep
           . ,(lambda (t)
                (set! columns
                      (append columns
                              (list (ly:context-property context 'currentCommandColumn)
                                    (ly:context-property context 'currentMusicalColumn))))))
          (finalize
           . ,(lambda (t)
                (for-each (lambda (c) (out (column c))) columns)
                (for-each (lambda (sp) (out (spanner sp))) spanners)))))
      'Sp_writer)
    (define (seq . elements) (make-music 'SequentialMusic 'elements elements))
    (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
    (define (skip n d) (make-music 'SkipEvent 'duration (ly:make-duration 0 0 n d)))
    (define (new type m)
      (make-music 'ContextSpeccedMusic 'context-type type 'create-new #t 'element m))
    (define (new-id type id m)
      (make-music 'ContextSpeccedMusic 'context-type type 'context-id id 'create-new #t
                  'element m))
    ;; A note of length num/den tagged with tags.
    (define (n num den . tags)
      (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0)
                  'duration (ly:make-duration 0 0 num den) 'tags tags))
    ;; A text script of length 1/16 (not a rhythmic event) tagged with tags.
    (define (txt . tags)
      (make-music 'TextScriptEvent 'length (ly:make-moment 1/16) 'tags tags))
    (define (chord . notes) (make-music 'EventChord 'elements notes))
    (define (grace m) (make-music 'GraceMusic 'element m))
    (define (section) (make-music 'SpacingSectionEvent))
    (define (pnd v)
      (make-music 'ContextSpeccedMusic 'context-type 'Score
                  'element (make-music 'PropertySet 'symbol 'proportionalNotationDuration
                                       'value v)))
    (define (unpnd)
      (make-music 'ContextSpeccedMusic 'context-type 'Score
                  'element (make-music 'PropertyUnset 'symbol 'proportionalNotationDuration)))";

/// LilyPond 2.26.0's records of the probe's score, one a line: each
/// column's `(label spacing shortest-playing-duration
/// shortest-starter-duration used)`, then each spanner's `(label no-cause?
/// left right wishes)`.
const SP_RECORDS: &str = r#"
((NonMusicalPaperColumn (0 0)) sp1 () () ())
((PaperColumn (0 0)) sp1 1/4 1/4 ())
((NonMusicalPaperColumn (1/4 0)) sp1 () () ())
((PaperColumn (1/4 0)) sp1 1/2 +inf.0 ())
((NonMusicalPaperColumn (1/2 0)) sp1 () () ())
((PaperColumn (1/2 0)) sp1 1/4 1/4 ())
((NonMusicalPaperColumn (5/8 0)) sp1 () () ())
((PaperColumn (5/8 0)) sp1 1/4 1/2 ())
((NonMusicalPaperColumn (3/4 0)) sp1 () () ())
((PaperColumn (3/4 0)) sp1 1/2 +inf.0 ())
((NonMusicalPaperColumn (9/8 0)) sp1 () () ())
((PaperColumn (9/8 0)) sp1 1 +inf.0 ())
((NonMusicalPaperColumn (3/2 0)) sp1 () () ())
((PaperColumn (3/2 0)) sp1 1/4 1/4 ())
((NonMusicalPaperColumn (7/4 -1/16)) sp1 () () ())
((PaperColumn (7/4 -1/16)) sp1 +inf.0 +inf.0 ())
((NonMusicalPaperColumn (7/4 0)) sp1 () () ())
((PaperColumn (7/4 0)) sp1 1/4 1/4 ())
((NonMusicalPaperColumn (2 0)) sp2 () () ())
((PaperColumn (2 0)) sp2 1/4 1/4 ())
((NonMusicalPaperColumn (9/4 0)) sp2 () () ())
((PaperColumn (9/4 0)) sp2 1/8 1/8 #t)
((NonMusicalPaperColumn (5/2 0)) sp2 () () ())
((PaperColumn (5/2 0)) sp2 1/4 1/4 ())
((NonMusicalPaperColumn (3 0)) sp2 () () ())
((PaperColumn (3 0)) sp2 0 0 #t)
((NonMusicalPaperColumn (13/4 0)) sp2 () () ())
((PaperColumn (13/4 0)) sp2 1/4 1/4 ())
((NonMusicalPaperColumn (15/4 0)) sp2 () () ())
((PaperColumn (15/4 0)) sp2 +inf.0 +inf.0 ())
(sp1 #t (NonMusicalPaperColumn (0 0)) (NonMusicalPaperColumn (2 0)) (s0 n0))
(sp2 #t (NonMusicalPaperColumn (2 0)) (NonMusicalPaperColumn (15/4 0)) (s4 n4))
"#;

/// `Spacing_engraver` (MS3 step 22, `spacing-engraver.cc`), layout's class
/// found before the music crate's placeholder: a SpacingSpanner with no
/// cause from the first command column, ended at a spacing section's (2),
/// where the next starts, and at the finalize's; its `wishes` the staff and
/// note spacings acknowledged while it runs; each column's `spacing` the
/// one running; each musical column's shortest starter duration that of
/// the notes starting (not a lyric syllable, a multi-measure grob, a grob
/// of a non-rhythmic event, even with a length, or of none, one of length 0 nor a grace note),
/// its shortest playing duration that and the full lengths of the notes
/// still playing (dropped when they end, 1/4, 1/2, 7/4); where
/// `proportionalNotationDuration` is not negative (1/8 at 9/4, 0 at 3), both
/// that, `used` `#t`, and the notes starting then counted at the next
/// timestep (5/2, 13/4). As LilyPond 2.26.0 gives the probe's score
/// (`$LILY_CORPUS/device-runs/2026-10-13-ms3-step22/session3-probe/`,
/// `probe-sp.out`).
#[test]
fn spacing_engraver_spaces_the_columns_as_lilypond_does() {
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::spacing_engraver::SPACING_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(SP_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, SP_PROBES);
    let music = eval(
        &mut vm,
        "(new 'Score
          (par (new-id 'Voice \"a\"
                (seq (chord (n 1 2 'h0 'NoteHead) (n 1 16 'l0 'LyricText)
                            (n 1 2 's0 'StaffSpacing) (n 1 2 'n0 'NoteSpacing))
                     (chord (n 1 1 'h1 'NoteHead)) (chord (n 1 4 'h2 'NoteHead))
                     (grace (n 1 16 'z1 'NoteHead)) (chord (n 1 4 'h3 'NoteHead))
                     (section)
                     (chord (n 1 4 'h4 'NoteHead) (n 1 4 's4 'StaffSpacing)
                            (n 1 4 'n4 'NoteSpacing))
                     (pnd 1/8) (chord (n 1 4 'h5 'NoteHead))
                     (unpnd) (chord (n 1 2 'h6 'NoteHead))
                     (pnd 0) (chord (n 1 4 'h7 'NoteHead))
                     (unpnd) (chord (n 1 2 'h8 'NoteHead))))
               (new-id 'Voice \"b\"
                (seq (chord (n 1 4 'r0 'Rest) (n 1 32 'm0 'MmHead))
                     (chord (txt 't1 'NoteHead) (n 1 4 'x1 'NoteHead 'nocause)
                            (n 0 1 'e1 'NoteHead) (skip 1 4))
                     (chord (n 1 4 'q2 'NoteHead))))
               (new-id 'Voice \"c\"
                (seq (skip 5 8) (chord (n 1 2 'd1 'NoteHead))))))",
    );
    vm.heap_mut().root(music);
    let classes = [&SCORE_ENGRAVER, &PAPER_COLUMN_ENGRAVER, &SPACING_ENGRAVER];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(show_of(&mut vm, "(kept)"), "()");
    let probed = eval(&mut vm, "(reverse probed)");
    let mut records = Vec::new();
    let mut p = probed;
    while let (Some(r), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
        records.push(show(&vm, r));
        p = rest;
    }
    let lilyponds: Vec<&str> = SP_RECORDS.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lilyponds.len(), 32);
    assert_eq!(records, lilyponds);
}

/// Global with NoteHead's, NoteColumn's, Rest's, Clef's, TextScript's and
/// the spacings' and columns' descriptions; a Score with
/// `Paper_column_engraver` and `Ns_writer`, holding Staves with
/// `createSpacing` `#t` and `Separating_line_group_engraver`, holding
/// Voices with `Ns_maker` and `Note_spacing_engraver`.
const NS_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((NoteHead (meta (name . NoteHead) (classes Item)
                        (interfaces grob-interface note-head-interface rhythmic-grob-interface
                                    rhythmic-head-interface)))
        (Rest (meta (name . Rest) (classes Item)
                    (interfaces grob-interface rest-interface rhythmic-grob-interface
                                rhythmic-head-interface)))
        (NoteColumn (axes 0)
                    (meta (name . NoteColumn) (classes Item)
                          (interfaces grob-interface axis-group-interface note-column-interface
                                      separation-item-interface)))
        (Clef (break-align-symbol . clef) (non-musical . #t)
              (meta (name . Clef) (classes Item)
                    (interfaces grob-interface break-aligned-interface clef-interface)))
        (TextScript (meta (name . TextScript) (classes Item)
                          (interfaces grob-interface text-script-interface)))
        (NoteSpacing (meta (name . NoteSpacing) (classes Item)
                           (interfaces grob-interface note-spacing-interface spacing-interface)))
        (StaffSpacing (non-musical . #t)
                      (meta (name . StaffSpacing) (classes Item)
                            (interfaces grob-interface spacing-interface
                                        staff-spacing-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (consists Paper_column_engraver) (consists Ns_writer))",
    "((context-name Staff) (translator-type Engraver_group) (accepts Voice)
      (assign createSpacing #t) (consists Separating_line_group_engraver))",
    "((context-name Voice) (translator-type Engraver_group) (consists Ns_maker)
      (consists Note_spacing_engraver))",
];

/// The probe's maker and writer and its music's helpers, written as its
/// `probe-ns.ly` writes them, each record kept in `probed`.
const NS_PROBES: &str = "
    (set-object-property! 'createSpacing 'translation-type? boolean?)
    (set-object-property! 'hasStaffSpacing 'translation-type? boolean?)
    (define-public X 0)
    (define-public probed '())
    (define (out x) (set! probed (cons x probed)))
    (define (mom m) (list (ly:moment-main m) (ly:moment-grace m)))
    (define labels '())
    (define made '())
    (define (made! g l) (set! labels (acons g l labels)))
    (define (gname g) (assq-ref (ly:grob-property g 'meta) 'name))
    (define (label g)
      (cond ((not (ly:grob? g)) g)
            ((assq-ref labels g) (assq-ref labels g))
            ((memq (gname g) '(PaperColumn NonMusicalPaperColumn))
             (list (gname g) (mom (ly:grob-property g 'when))))
            (else (gname g))))
    (define (arr x) (if (ly:grob-array? x) (map label (ly:grob-array->list x)) x))
    (define (record g)
      (list (label g) (label (ly:grob-property g 'cause)) (label (ly:grob-parent g X))
            (arr (ly:grob-object g 'left-items)) (arr (ly:grob-object g 'right-items))))
    (define (tags ev) (ly:event-property ev 'tags))
    ;; A grob of each note heard, in the order heard, caused by it, named by
    ;; its second tag and labelled by its first.
    (ly:register-translator
      (lambda (context)
        (let ((heard '()))
          `((listeners
             (note-event . ,(lambda (engraver event) (set! heard (cons event heard)))))
            (process-music
             . ,(lambda (engraver)
                  (for-each
                   (lambda (ev)
                     (let ((g (ly:engraver-make-grob engraver (cadr (tags ev)) ev)))
                       (made! g (car (tags ev)))))
                   (reverse heard))
                  (set! heard '()))))))
      'Ns_maker)
    ;; Each NoteSpacing labelled by its cause's label; their records at the
    ;; finalize, after the Voices'.
    (ly:register-translator
      (lambda (context)
        `((acknowledgers
           (note-spacing-interface
            . ,(lambda (t g s)
                 (made! g (list 'ns (label (ly:grob-property g 'cause))))
                 (set! made (append made (list g))))))
          (finalize . ,(lambda (t) (for-each (lambda (g) (out (record g))) made)))))
      'Ns_writer)
    (define (seq . elements) (make-music 'SequentialMusic 'elements elements))
    (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
    (define (s) (make-music 'SkipEvent 'duration (ly:make-duration 2)))
    (define (new-id type id m)
      (make-music 'ContextSpeccedMusic 'context-type type 'context-id id 'create-new #t
                  'element m))
    (define (change id) (make-music 'ContextChange 'change-to-type 'Staff 'change-to-id id))
    ;; A quarter note labelled l whose grob is name.
    (define (n l name)
      (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration 2)
                  'tags (list l name)))
    (define (chord . notes) (make-music 'EventChord 'elements notes))";

/// LilyPond 2.26.0's records of the probe's score, one a line.
const NS_RECORDS: &str = "
((ns h0) h0 (PaperColumn (0 0)) (h0 nc0) (nc1 r1))
((ns hw0) hw0 (PaperColumn (0 0)) (hw0) ((NonMusicalPaperColumn (1/2 0)) (NonMusicalPaperColumn (1 0))))
((ns u0) u0 (PaperColumn (0 0)) (u0) (u1 (NonMusicalPaperColumn (1/2 0))))
((ns nc1) nc1 (PaperColumn (1/4 0)) (nc1 r1) ((NonMusicalPaperColumn (1/2 0)) h4 (NonMusicalPaperColumn (1 0))))
((ns u1) u1 (PaperColumn (1/4 0)) (u1) ())
((ns h4) h4 (PaperColumn (1 0)) (h4) (h5 (NonMusicalPaperColumn (3/2 0))))
((ns h5) h5 (PaperColumn (5/4 0)) (h5) (h6))
((ns h6) h6 (PaperColumn (3/2 0)) (h6) ((NonMusicalPaperColumn (7/4 0))))
";

/// `Note_spacing_engraver` (MS3 step 22, `note-spacing-engraver.cc`),
/// layout's class found before the music crate's placeholder: a
/// NoteSpacing at each timestep of a Voice with a note column or rhythmic
/// grob (none for a TextScript, t3, or a Clef), caused by the first, on the
/// musical column, its `left-items` all of them; the Voice's last note
/// spacing's `right-items` them (across `\change Staff`: h5, h6, u1), and
/// the command column of each later timestep with a staff spacing, added
/// to the last one made under the Voice's present Staff (h4's at 3/2, back
/// in "a"; none in "b" at 5/4, where the Voice had made none); at a
/// Voice's finalize the command column added to its Staff's last one
/// with no `right-items` (h6's) and not to one with some (hw0's, u0's).
/// As LilyPond 2.26.0 gives the probe's score
/// (`$LILY_CORPUS/device-runs/2026-10-13-ms3-step22/session4-probe/`,
/// `probe-ns.out`).
#[test]
fn note_spacing_engraver_links_the_note_columns_as_lilypond_does() {
    use lily_engine_layout::note_spacing_engraver::NOTE_SPACING_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    use lily_engine_layout::separating_line_group_engraver::SEPARATING_LINE_GROUP_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(NS_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, NS_PROBES);
    let music = eval(
        &mut vm,
        "(new-id 'Score \"s\"
          (par (new-id 'Staff \"a\"
                (par (new-id 'Voice \"c\"
                      (seq (chord (n 'c0 'Clef)) (s) (chord (n 'c2 'Clef)) (s)
                           (chord (n 'c4 'Clef)) (s) (chord (n 'c6 'Clef)) (s) (s)))
                     (new-id 'Voice \"v\"
                      (seq (chord (n 'h0 'NoteHead) (n 'nc0 'NoteColumn))
                           (chord (n 'nc1 'NoteColumn) (n 'r1 'Rest)) (s)
                           (chord (n 't3 'TextScript)) (chord (n 'h4 'NoteHead))
                           (change \"b\") (chord (n 'h5 'NoteHead))
                           (change \"a\") (chord (n 'h6 'NoteHead))))
                     (new-id 'Voice \"w\"
                      (seq (chord (n 'hw0 'NoteHead)) (s) (s) (s) (s)))
                     (new-id 'Voice \"u\"
                      (seq (chord (n 'u0 'NoteHead)) (change \"b\") (chord (n 'u1 'NoteHead))
                           (change \"a\") (s)))))
               (new-id 'Staff \"b\"
                (new-id 'Voice \"d\"
                 (seq (s) (s) (s) (s) (s) (chord (n 'cb5 'Clef)) (s) (s) (s))))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &SEPARATING_LINE_GROUP_ENGRAVER,
        &NOTE_SPACING_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(show_of(&mut vm, "(kept)"), "()");
    let probed = eval(&mut vm, "(reverse probed)");
    let mut records = Vec::new();
    let mut p = probed;
    while let (Some(r), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
        records.push(show(&vm, r));
        p = rest;
    }
    let lilyponds: Vec<&str> = NS_RECORDS.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lilyponds.len(), 8);
    assert_eq!(records, lilyponds);
}

/// Global with Script's, ScriptColumn's and the columns' descriptions; a
/// Score with `Paper_column_engraver` and `Nmsc_writer`, holding a Staff
/// with `Non_musical_script_column_engraver` and `Nmsc_killer`, whose
/// Voices have `Nmsc_maker`.
const NMSC_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((Script (meta (name . Script) (classes Item)
                      (interfaces grob-interface script-interface side-position-interface)))
        (ScriptColumn (meta (name . ScriptColumn) (classes Item)
                            (interfaces grob-interface script-column-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (consists Paper_column_engraver) (consists Nmsc_writer))",
    "((context-name Staff) (translator-type Engraver_group) (accepts Voice)
      (consists Non_musical_script_column_engraver) (consists Nmsc_killer))",
    "((context-name Voice) (translator-type Engraver_group) (consists Nmsc_maker))",
];

/// The probe's maker, killer and writer and its music's helpers, written
/// as its `probe-nmsc.ly` writes them, `point-stencil` as `stencil.scm`
/// makes it and `grob::is-live?` as `output-lib.scm` defines it, each
/// record kept in `probed`.
const NMSC_PROBES: &str = "
    (set-object-property! 'side-axis 'backend-type? number?)
    (set-object-property! 'script-priority 'backend-type? number?)
    (set-object-property! 'stencil 'backend-type? ly:stencil?)
    (set-object-property! 'non-musical 'backend-type? boolean?)
    (define-public X 0)
    (define-public Y 1)
    (define-public point-stencil (ly:make-stencil \"\" '(0 . 0) '(0 . 0)))
    (define-public (grob::is-live? grob) (pair? (ly:grob-basic-properties grob)))
    (define-public probed '())
    (define (out x) (set! probed (cons x probed)))
    (define (mom m) (list (ly:moment-main m) (ly:moment-grace m)))
    (define labels '())
    (define made '())
    (define columns '())
    (define (made! g l) (set! labels (acons g l labels)))
    (define (gname g) (assq-ref (ly:grob-property g 'meta) 'name))
    (define (label g)
      (cond ((not (ly:grob? g)) g)
            ((assq-ref labels g) (assq-ref labels g))
            ((memq (gname g) '(PaperColumn NonMusicalPaperColumn))
             (list (gname g) (mom (ly:grob-property g 'when))))
            (else (gname g))))
    (define (arr x) (if (ly:grob-array? x) (map label (ly:grob-array->list x)) x))
    (define (tags ev) (ly:event-property ev 'tags))
    ;; A Script of each note heard, in the order heard, caused by it,
    ;; labelled by its first tag, its side-axis and script-priority the
    ;; next two unless #f, its stencil point-stencil if tagged stencil,
    ;; else #f, on the command column if tagged cmd.
    (ly:register-translator
      (lambda (context)
        (let ((heard '()))
          `((listeners
             (note-event . ,(lambda (engraver event) (set! heard (cons event heard)))))
            (process-music
             . ,(lambda (engraver)
                  (for-each
                   (lambda (ev)
                     (let* ((t (tags ev))
                            (g (ly:engraver-make-grob engraver 'Script ev)))
                       (if (list-ref t 1) (ly:grob-set-property! g 'side-axis (list-ref t 1)))
                       (if (list-ref t 2)
                           (ly:grob-set-property! g 'script-priority (list-ref t 2)))
                       (ly:grob-set-property! g 'stencil (if (memq 'stencil t) point-stencil #f))
                       (if (memq 'cmd t)
                           (ly:grob-set-parent!
                            g X (ly:context-property context 'currentCommandColumn)))
                       (made! g (car t))
                       (set! made (append made (list g)))))
                   (reverse heard))
                  (set! heard '()))))))
      'Nmsc_maker)
    ;; Each live script whose label starts with k killed at each
    ;; process-acknowledged.
    (ly:register-translator
      (lambda (context)
        `((process-acknowledged
           . ,(lambda (engraver)
                (for-each
                 (lambda (g)
                   (if (and (grob::is-live? g)
                            (char=? #\\k (string-ref (symbol->string (label g)) 0)))
                       (ly:grob-suicide! g)))
                 made)))))
      'Nmsc_killer)
    ;; Each ScriptColumn labelled by its cause's label; at the finalize each
    ;; script's script-column, then each column's record.
    (ly:register-translator
      (lambda (context)
        `((acknowledgers
           (script-column-interface
            . ,(lambda (t g s)
                 (made! g (list 'sc (label (ly:grob-property g 'cause))))
                 (set! columns (append columns (list g))))))
          (finalize
           . ,(lambda (t)
                (for-each
                 (lambda (g) (out (list (label g) (label (ly:grob-object g 'script-column)))))
                 made)
                (for-each
                 (lambda (g)
                   (out (list (label g) (label (ly:grob-property g 'cause))
                              (ly:grob-property g 'non-musical) (label (ly:grob-parent g X))
                              (arr (ly:grob-object g 'scripts)))))
                 columns)))))
      'Nmsc_writer)
    (define (seq . elements) (make-music 'SequentialMusic 'elements elements))
    (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
    (define (s) (make-music 'SkipEvent 'duration (ly:make-duration 2)))
    (define (new-id type id m)
      (make-music 'ContextSpeccedMusic 'context-type type 'context-id id 'create-new #t
                  'element m))
    (define (n . tags)
      (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration 2)
                  'tags tags))
    (define (chord . notes) (make-music 'EventChord 'elements notes))";

/// LilyPond 2.26.0's records of the probe's score, one a line.
const NMSC_RECORDS: &str = "
(s1 (sc s1))
(s2 (sc s1))
(m1 ())
(x1 ())
(m2 ())
(a1 ())
(a2 (sc a1))
(a3 ())
(k1 ())
(n1 ())
(n2 ())
(w1 (sc w1))
(w2 (sc w1))
((sc s1) s1 #t (NonMusicalPaperColumn (0 0)) (s1 s2))
((sc a1) a1 #t (NonMusicalPaperColumn (1/2 0)) (a2))
((sc w1) w1 #t (NonMusicalPaperColumn (3/4 0)) (w1 w2))
";

/// `Non_musical_script_column_engraver` (MS3 step 22,
/// `non-musical-script-column-engraver.cc`), layout's class found before
/// the music crate's placeholder: a ScriptColumn at each timestep of the
/// Staff with two or more non-musical scripts (none at 1/4, with one, m1
/// and m2 musical), caused by the first, `non-musical` `#t`, on the
/// command column, its `scripts` the live ones on the Y axis with a
/// `script-priority` (not a1, on X, a3, with none, k1, killed after it
/// was acknowledged, n1 and n2, on no axis), from both Voices (w1, w2);
/// and LilyPond's programming error for n2, on no axis with a stencil. As
/// LilyPond 2.26.0 gives the probe's score
/// (`$LILY_CORPUS/device-runs/2026-10-13-ms3-step22/session5-probe/`,
/// `probe-nmsc.out`, `probe-nmsc.log`).
#[test]
fn non_musical_script_column_engraver_collects_scripts_as_lilypond_does() {
    use lily_engine_layout::non_musical_script_column_engraver::NON_MUSICAL_SCRIPT_COLUMN_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(NMSC_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, NMSC_PROBES);
    let music = eval(
        &mut vm,
        "(new-id 'Score \"s\"
          (new-id 'Staff \"a\"
           (par (new-id 'Voice \"v\"
                 (seq (chord (n 's1 Y 0 'cmd) (n 's2 Y 1 'cmd) (n 'm1 Y 0))
                      (chord (n 'x1 Y 0 'cmd) (n 'm2 Y 0))
                      (chord (n 'a1 X 0 'cmd) (n 'a2 Y 0 'cmd) (n 'a3 Y #f 'cmd)
                             (n 'k1 Y 0 'cmd) (n 'n1 #f 0 'cmd) (n 'n2 #f 0 'stencil 'cmd))
                      (chord (n 'w1 Y 2 'cmd))))
                (new-id 'Voice \"w\"
                 (seq (s) (s) (s) (chord (n 'w2 Y 3 'cmd)))))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &NON_MUSICAL_SCRIPT_COLUMN_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    let error = "(programming-error \"no side-axis setting found for grob Script.\")";
    assert_eq!(show_of(&mut vm, "(kept)"), format!("({error})"));
    let probed = eval(&mut vm, "(reverse probed)");
    let mut records = Vec::new();
    let mut p = probed;
    while let (Some(r), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
        records.push(show(&vm, r));
        p = rest;
    }
    let lilyponds: Vec<&str> = NMSC_RECORDS.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lilyponds.len(), 16);
    assert_eq!(records, lilyponds);
}

/// Global with GraceSpacing's and the columns' descriptions; a Score with
/// `Paper_column_engraver`, `Grace_spacing_engraver` and `Gs_writer`,
/// holding Voices without translators.
const GS_DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((GraceSpacing (meta (name . GraceSpacing) (classes Spanner)
                            (interfaces grob-interface spanner-interface grace-spacing-interface
                                        spacing-options-interface)))
        (NonMusicalPaperColumn (axes 0) (line-break-permission . allow) (non-musical . #t)
                               (page-break-permission . allow)
                               (meta (name . NonMusicalPaperColumn) (classes Paper_column)
                                     (interfaces grob-interface axis-group-interface
                                                 separation-item-interface)))
        (PaperColumn (axes 0)
                     (meta (name . PaperColumn) (classes Paper_column)
                           (interfaces grob-interface axis-group-interface
                                       separation-item-interface)))
        (System (axes 0 1)
                (meta (name . System) (classes System)
                      (interfaces grob-interface axis-group-interface system-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice)
      (consists Paper_column_engraver) (consists Grace_spacing_engraver) (consists Gs_writer))",
    "((context-name Voice) (translator-type Engraver_group))",
];

/// The probe's writer and its music's helpers, written as its
/// `probe-gs.ly` writes them, each record kept in `probed`.
const GS_PROBES: &str = "
    (define-public LEFT -1)
    (define-public RIGHT 1)
    (define-public probed '())
    (define (out x) (set! probed (cons x probed)))
    (define (mom m) (list (ly:moment-main m) (ly:moment-grace m)))
    (define labels '())
    (define spacings '())
    (define columns '())
    (define (made! g l) (set! labels (acons g l labels)))
    (define (gname g) (assq-ref (ly:grob-property g 'meta) 'name))
    (define (label g)
      (cond ((not (ly:grob? g)) g)
            ((assq-ref labels g) (assq-ref labels g))
            ((memq (gname g) '(PaperColumn NonMusicalPaperColumn))
             (list (gname g) (mom (ly:grob-property g 'when))))
            (else (gname g))))
    (define (arr x) (if (ly:grob-array? x) (map label (ly:grob-array->list x)) x))
    ;; Each GraceSpacing labelled gs1, gs2, ...; each timestep's musical
    ;; column kept at its stop; at the finalize each column's grace-spacing,
    ;; then each GraceSpacing's record.
    (ly:register-translator
      (lambda (context)
        `((acknowledgers
           (grace-spacing-interface
            . ,(lambda (t g s)
                 (set! spacings (append spacings (list g)))
                 (made! g (string->symbol (format #f \"gs~a\" (length spacings)))))))
          (stop-translation-timestep
           . ,(lambda (t)
                (set! columns
                      (append columns
                              (list (ly:context-property context 'currentMusicalColumn))))))
          (finalize
           . ,(lambda (t)
                (for-each
                 (lambda (c) (out (list (label c) (label (ly:grob-object c 'grace-spacing)))))
                 columns)
                (for-each
                 (lambda (g)
                   (out (list (label g) (null? (ly:grob-property g 'cause))
                              (label (ly:spanner-bound g LEFT)) (label (ly:spanner-bound g RIGHT))
                              (arr (ly:grob-object g 'columns)))))
                 spacings)))))
      'Gs_writer)
    (define (seq . elements) (make-music 'SequentialMusic 'elements elements))
    (define (par . elements) (make-music 'SimultaneousMusic 'elements elements))
    (define (skip n d) (make-music 'SkipEvent 'duration (ly:make-duration 0 0 n d)))
    (define (new-id type id m)
      (make-music 'ContextSpeccedMusic 'context-type type 'context-id id 'create-new #t
                  'element m))
    (define (n num den)
      (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0)
                  'duration (ly:make-duration 0 0 num den)))
    (define (chord . notes) (make-music 'EventChord 'elements notes))
    (define (grace . elements) (make-music 'GraceMusic 'element (apply seq elements)))";

/// LilyPond 2.26.0's records of the probe's score, one a line.
const GS_RECORDS: &str = "
((PaperColumn (0 -1/8)) gs1)
((PaperColumn (0 -1/16)) gs1)
((PaperColumn (0 0)) gs1)
((PaperColumn (1/4 0)) ())
((PaperColumn (1/2 -1/16)) gs2)
((PaperColumn (1/2 0)) gs2)
((PaperColumn (3/4 0)) ())
((PaperColumn (1 0)) ())
((PaperColumn (9/8 -1/16)) gs3)
((PaperColumn (9/8 0)) gs3)
((PaperColumn (5/4 0)) ())
((PaperColumn (3/2 -1/16)) gs4)
((PaperColumn (3/2 0)) gs4)
((PaperColumn (7/4 0)) ())
(gs1 #t (PaperColumn (0 -1/8)) (PaperColumn (0 0)) ((PaperColumn (0 -1/8)) (PaperColumn (0 -1/16)) (PaperColumn (0 0))))
(gs2 #t (PaperColumn (1/2 -1/16)) (PaperColumn (1/2 0)) ((PaperColumn (1/2 -1/16)) (PaperColumn (1/2 0))))
(gs3 #t (PaperColumn (9/8 -1/16)) (PaperColumn (9/8 0)) ((PaperColumn (9/8 -1/16)) (PaperColumn (9/8 0))))
(gs4 #t (PaperColumn (3/2 -1/16)) (PaperColumn (3/2 0)) ((PaperColumn (3/2 -1/16)) (PaperColumn (3/2 0))))
";

/// `Grace_spacing_engraver` (MS3 step 22, `grace-spacing-engraver.cc`),
/// layout's class found before the music crate's placeholder: a
/// GraceSpacing with no cause at each grace timestep after one that was
/// not (the score's first among them), its `columns` the musical columns
/// of its grace timesteps and of the one after them, each its column's
/// `grace-spacing`, its bounds the first of them and the last; from two
/// Voices' graces (w's at 9/8, after a main timestep, as `\afterGrace`
/// puts one). As LilyPond 2.26.0 gives the probe's score
/// (`$LILY_CORPUS/device-runs/2026-10-13-ms3-step22/session5-probe/`,
/// `probe-gs.out`).
#[test]
fn grace_spacing_engraver_spaces_the_graces_as_lilypond_does() {
    use lily_engine_layout::grace_spacing_engraver::GRACE_SPACING_ENGRAVER;
    use lily_engine_layout::paper_column_engraver::PAPER_COLUMN_ENGRAVER;
    use lily_engine_layout::score_engraver::SCORE_ENGRAVER;
    let Some((mut vm, layout)) = vm_with(GS_DEFS) else {
        return;
    };
    keep_diagnostics(&mut vm);
    eval(&mut vm, GS_PROBES);
    let music = eval(
        &mut vm,
        "(new-id 'Score \"s\"
          (par (new-id 'Voice \"v\"
                (seq (grace (chord (n 1 16)) (chord (n 1 16)))
                     (chord (n 1 4)) (chord (n 1 4))
                     (grace (chord (n 1 16))) (chord (n 1 4)) (chord (n 1 4)) (chord (n 1 2))
                     (grace (chord (n 1 16))) (chord (n 1 4))))
               (new-id 'Voice \"w\"
                (seq (skip 9 8) (grace (chord (n 1 16))) (skip 1 8)))))",
    );
    vm.heap_mut().root(music);
    let classes = [
        &SCORE_ENGRAVER,
        &PAPER_COLUMN_ENGRAVER,
        &GRACE_SPACING_ENGRAVER,
    ];
    let done = run_translator_with(&mut vm, music, layout, None, &classes)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    assert_eq!(show_of(&mut vm, "(kept)"), "()");
    let probed = eval(&mut vm, "(reverse probed)");
    let mut records = Vec::new();
    let mut p = probed;
    while let (Some(r), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
        records.push(show(&vm, r));
        p = rest;
    }
    let lilyponds: Vec<&str> = GS_RECORDS.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lilyponds.len(), 18);
    assert_eq!(records, lilyponds);
}

/// Part 2's 36 translator classes (MS3 steps 15–22), by name: the `part2`
/// rows of `$LILY_CORPUS/ms3-part2-split/classes.txt`.
const PART2: [&str; 36] = [
    "Accidental_engraver",
    "Axis_group_engraver",
    "Bar_engraver",
    "Bar_number_engraver",
    "Break_align_engraver",
    "Clef_engraver",
    "Dot_column_engraver",
    "Dots_engraver",
    "Dynamic_align_engraver",
    "Dynamic_engraver",
    "Forbid_line_break_engraver",
    "Grace_spacing_engraver",
    "Grob_pq_engraver",
    "Keep_alive_together_engraver",
    "Key_engraver",
    "Ledger_line_engraver",
    "Mark_tracking_translator",
    "Non_musical_script_column_engraver",
    "Note_heads_engraver",
    "Note_spacing_engraver",
    "Ottava_spanner_engraver",
    "Paper_column_engraver",
    "Pure_from_neighbor_engraver",
    "Rest_engraver",
    "Rhythmic_column_engraver",
    "Script_row_engraver",
    "Separating_line_group_engraver",
    "Spacing_engraver",
    "Spanner_break_forbid_engraver",
    "Staff_symbol_engraver",
    "Stem_engraver",
    "System_start_delimiter_engraver",
    "Text_engraver",
    "Time_signature_engraver",
    "Vertical_align_engraver",
    "Volta_engraver",
];

/// Each of part 2's 36 classes resolves in a `\layout` translation to a
/// class with work (MS3 step 22): a name is found among the classes the
/// core passes at its `\layout` call, layout's [`CLASSES`], before the
/// music crate's (`translator.rs`' `resolve`), and has a state and a method,
/// an event or an interface. 35 are layout's, each shadowing the music
/// crate's placeholder of its name, and `Mark_tracking_translator` the
/// music crate's own (MS3 step 20); layout's ten others are
/// `Score_engraver`'s layout part, `Collision_engraver` and
/// `Rest_collision_engraver` (MS3 step 41, the 36th and 37th placeholders
/// layout shadows), `Multi_measure_rest_engraver`, `Span_bar_engraver`
/// and `Span_bar_stub_engraver` (MS3 step 42, the 38th to 40th),
/// `Instrument_name_engraver` (MS3 step 43, the 41st), `Tie_engraver`
/// (MS4's), `Font_size_engraver` and `Staff_collecting_engraver` (MS3
/// step 48, the 42nd and 43rd), `Horizontal_bracket_engraver` (MS3
/// step 49, the 44th), `Text_spanner_engraver` and
/// `Note_head_line_engraver` (MS3 step 52, the 45th and 46th),
/// `Script_engraver`, `Script_column_engraver`, `Fingering_engraver`,
/// `New_fingering_engraver` and `Fingering_column_engraver` (MS3 step 53,
/// the 47th to 51st) and `Arpeggio_engraver`, `Span_arpeggio_engraver` and
/// `Metronome_mark_engraver` (MS3 step 54, the 52nd to 54th), since
/// MS3 step 55 `Apply_output_engraver`, in the image's Scheme engraver's
/// place and no placeholder's (D213), and since MS4 step 2
/// `Beam_engraver` and `Grace_beam_engraver`, in the music crate's
/// melisma classes' places, and `Auto_beam_engraver` and
/// `Grace_auto_beam_engraver`, the 55th and 56th, and since MS4 step 4
/// `Beam_collision_engraver` and `Tweak_engraver`, the 57th and 58th, and
/// since step 5 `Chord_tremolo_engraver`, the 59th, and since step 7
/// `Slur_engraver`, in the melisma class's place, and
/// `Phrasing_slur_engraver`, the 60th, and since step 10
/// `Laissez_vibrer_engraver` and `Pitch_squash_engraver` (R9's, the
/// operator's answer for T29), the 61st and 62nd, and since step 12
/// `Tuplet_engraver` and since step 13 `Drum_notes_engraver`, the 63rd
/// and 64th, and since step 15 the lyric engravers (`Lyric_engraver`,
/// `Extender_engraver`, `Hyphen_engraver`, `Stanza_number_engraver`,
/// `Stanza_number_align_engraver`), the 65th to 69th, and since MS8 step 7
/// `Repeat_tie_engraver` and `Mark_engraver`, the 70th and 71st, and
/// `Jump_engraver`, the 72nd, and step 7's part 2's eight, the 73rd to
/// 80th, and step 8's part 1's five, the 81st to 85th, and its part 2's
/// eight, the 86th to 93rd, and step 9's part 1's three, the 94th to 96th,
/// and its part 2's two, the 97th and 98th, and its part 3's
/// `Page_turn_engraver`, the 99th. The music
/// crate's `CLASSES` keeps its 128 classes, 117 of them placeholders (40
/// shadowed by layout's since MS3 step 42, 41 since step 43, 43 since step
/// 48, 44 since step 49, 46 since step 52, 51 since step 53, 54 since step
/// 54, 56 since MS4 step 2, 58 since step 4, 59 since step 5, 60 since step 7, 62 since step 10, 63 since step 12, 64 since step 13, 69 since step 15, 71 since MS8 step 7, 72 later in it, 80 in its part 2, 85 in step 8's part 1, 93 in its part 2, 96 in step 9's part 1, 98 in its part 2, 99 in its part 3), and `grob.rs`' `PRIMS` its 39 (221 until MS3 step 26, 213 until step 27, 203, 195, 191 and 187 in step 28, 185, 184, 183 and 182 in step 30, 178 until step 31, 176 until step 33, 169, 164 and 157 in step 37, 156 until step 38, 142 until step 39, 137 in step 39, 135, 131 and 128 in step 40, 127, 121 and 120 in step 41, 119 and 115 in step 42, 114 until step 48, 112 until step 49, 106 until step 52, 99 until step 53, 94 until step 54, 85 until step 62, 84 until MS4 step 3, 75 until step 4, 74 until step 5, 64 until step 7, 59 until step 8, 58 until step 10, 50 until step 12, 45 until step 13, 42 until step 15). `tools/compare/test/grobs.test.mjs` runs the
/// 36 at work, natively and in wasm.
///
/// [`CLASSES`]: lily_engine_layout::CLASSES
#[test]
fn part_2s_36_classes_resolve_to_classes_with_work() {
    let layout = lily_engine_layout::CLASSES;
    let music = &lily_engine_music::CLASSES;
    let mut layouts = Vec::new();
    for name in PART2 {
        let class = layout
            .iter()
            .copied()
            .find(|c| c.name == name)
            .or_else(|| music.iter().find(|c| c.name == name))
            .unwrap_or_else(|| panic!("{name} resolves to no class"));
        assert!(class.make.is_some(), "{name} has no state");
        assert!(
            class.methods != 0 || !class.hears.is_empty() || !class.acknowledges.is_empty(),
            "{name} has no work"
        );
        if layout.iter().any(|c| std::ptr::eq(*c, class)) {
            layouts.push(name);
        }
        let own = music.iter().find(|c| c.name == name).unwrap();
        assert_eq!(
            own.make.is_none(),
            name != "Mark_tracking_translator",
            "{name}: the music crate's class"
        );
    }
    assert_eq!(layouts.len(), 35);
    assert!(!layouts.contains(&"Mark_tracking_translator"));
    let others: Vec<&str> = layout
        .iter()
        .map(|c| c.name)
        .filter(|name| !PART2.contains(name))
        .collect();
    assert_eq!(
        others,
        [
            "Score_engraver",
            "Collision_engraver",
            "Rest_collision_engraver",
            "Multi_measure_rest_engraver",
            "Span_bar_engraver",
            "Span_bar_stub_engraver",
            "Instrument_name_engraver",
            "Tie_engraver",
            "Laissez_vibrer_engraver",
            "Repeat_tie_engraver",
            "Pitch_squash_engraver",
            "Tuplet_engraver",
            "Drum_notes_engraver",
            "Lyric_engraver",
            "Extender_engraver",
            "Hyphen_engraver",
            "Stanza_number_engraver",
            "Stanza_number_align_engraver",
            "Horizontal_bracket_engraver",
            "Text_spanner_engraver",
            "Note_head_line_engraver",
            "Script_engraver",
            "Script_column_engraver",
            "Fingering_engraver",
            "New_fingering_engraver",
            "Fingering_column_engraver",
            "Arpeggio_engraver",
            "Span_arpeggio_engraver",
            "Metronome_mark_engraver",
            "Mark_engraver",
            "Jump_engraver",
            "Percent_repeat_engraver",
            "Double_percent_repeat_engraver",
            "Slash_repeat_engraver",
            "Caesura_engraver",
            "Cue_clef_engraver",
            "Instrument_switch_engraver",
            "Part_combine_engraver",
            "Parenthesis_engraver",
            "Note_name_engraver",
            "Melody_engraver",
            "Piano_pedal_engraver",
            "Piano_pedal_align_engraver",
            "Pitched_trill_engraver",
            "Grid_point_engraver",
            "Grid_line_span_engraver",
            "Completion_heads_engraver",
            "Completion_rest_engraver",
            "Ambitus_engraver",
            "Cluster_spanner_engraver",
            "Balloon_engraver",
            "Footnote_engraver",
            "Ligature_bracket_engraver",
            "Mensural_ligature_engraver",
            "Episema_engraver",
            "Vaticana_ligature_engraver",
            "Kievan_ligature_engraver",
            "Page_turn_engraver",
            "Beam_engraver",
            "Grace_beam_engraver",
            "Slur_engraver",
            "Phrasing_slur_engraver",
            "Auto_beam_engraver",
            "Grace_auto_beam_engraver",
            "Beam_collision_engraver",
            "Tweak_engraver",
            "Chord_tremolo_engraver",
            "Font_size_engraver",
            "Staff_collecting_engraver",
            "Apply_output_engraver"
        ]
    );
    assert_eq!(layout.len(), 105);

    assert_eq!(music.len(), 128);
    // 105 since MS5 step 5's Drum_note_performer, the last performer, 106
    // since step 4's two (Lyric_performer, Mark_performer), 108 since step
    // 3's four (Key_performer, Time_signature_performer,
    // Midi_control_change_performer, Piano_pedal_performer), 112 since step
    // 2's Dynamic_performer, after step 1's four.
    assert_eq!(music.iter().filter(|c| c.make.is_none()).count(), 105);
    // The placeholders layout shadows: 40 since MS3 step 42, 41 since step
    // 43, 43 since step 48, 44 since step 49, 46 since step 52, 51 since
    // step 53, 54 since step 54, 56 since MS4 step 2 (`Auto_beam_engraver`
    // and `Grace_auto_beam_engraver`; the two beam engravers shadow the
    // music crate's classes that keep `beamMelismaBusy`), 58 since step 4
    // (`Beam_collision_engraver`, `Tweak_engraver`), 59 since step 5
    // (`Chord_tremolo_engraver`), 60 since step 7
    // (`Phrasing_slur_engraver`; `Slur_engraver` shadows the music crate's
    // class that keeps `slurMelismaBusy`), 62 since step 10
    // (`Laissez_vibrer_engraver`, `Pitch_squash_engraver`), 63 since step
    // 12 (`Tuplet_engraver`), 64 since step 13 (`Drum_notes_engraver`), 69
    // since step 15 (the lyric engravers), 71 since MS8 step 7
    // (`Repeat_tie_engraver`, `Mark_engraver`), 72 with `Jump_engraver`, 80
    // with step 7's part 2 (the three percent repeats' engravers,
    // `Caesura_engraver`, `Cue_clef_engraver`, `Instrument_switch_engraver`,
    // `Part_combine_engraver`, `Parenthesis_engraver`), 85 with step 8's
    // part 1 (`Note_name_engraver`, `Melody_engraver`, the two piano pedal
    // engravers, `Pitched_trill_engraver`), 93 with its part 2 (the two
    // grid engravers, the two completion engravers, `Ambitus_engraver`,
    // `Cluster_spanner_engraver`, `Balloon_engraver`, `Footnote_engraver`),
    // 96 with step 9's part 1 (`Ligature_bracket_engraver`,
    // `Mensural_ligature_engraver`, `Episema_engraver`), 98 with its part 2
    // (`Vaticana_ligature_engraver`, `Kievan_ligature_engraver`), 99 with
    // its part 3 (`Page_turn_engraver`).
    let shadowed = music
        .iter()
        .filter(|c| c.make.is_none() && layout.iter().any(|l| l.name == c.name))
        .count();
    assert_eq!(shadowed, 99);
    // `grob.rs`' placeholders, the second of the music crate's primitives.
    let grob_prims = lily_engine_music::EXTENSION.prims[1];
    assert_eq!(grob_prims[0].name, "ly:balloon-interface::print");
    assert_eq!(grob_prims.len(), 39);
}
