//! N3's instances (MS2 step 22, 04 §7, D73): contexts made, found and
//! removed as `context.cc` does it, on hand-made definitions of LilyPond's
//! shape (Global, an adopting Score, StaffGroup, Staff, Voice, CueVoice,
//! Lyrics), in a VM with only the music crate: the order and the ids of
//! the contexts a search makes, the events that make and announce them,
//! properties and their checks, override stacks with `\once`, removal,
//! event classes and dispatchers' priorities. The translation traces of
//! `tools/compare/trace.mjs` check the same against LilyPond from step 23.
use lily_engine_music::{
    Arena, ClassSet, CtxId, Dir, EventClasses, GLOBAL, OutputDef, Translation, check_removal,
    class_index, create_unique_context, find_create_context, get_default_interpreter,
    get_user_accessible_interpreter, make_context_def, send,
};
use lily_engine_scheme::{Value, Vm};

/// A VM with the music crate's functions, `(lily)` current.
fn vm() -> Vm {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    vm.eval_text("(set-current-module (resolve-module '(lily)))", None)
        .unwrap();
    vm
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

/// The definitions, each a list of mods, in an output definition.
const DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions ((NoteHead . ((color . red) (details . ((a . 1))))) (Stem . ((length . 7))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff) (accepts StaffGroup)
      (accepts Lyrics) (default-child Staff) (assign measureLength 1) (assign timing #t))",
    "((context-name StaffGroup) (accepts Staff) (default-child Staff))",
    "((context-name Staff) (accepts CueVoice) (accepts Voice) (default-child Voice)
      (assign tonic c) (push NoteHead blue color))",
    "((context-name Voice) (alias Staff-voice))",
    "((context-name CueVoice) (alias Voice))",
    "((context-name Lyrics))",
];

/// A translation of [`DEFS`], rooted, with `log`, a list in `(lily)` that
/// a listener on Global's events below conses each `CreateContext` and
/// `AnnounceNewContext` onto, as `(class . name)`.
fn translation(vm: &mut Vm) -> Translation {
    let scope = vm.make_module(&["lily"]).unwrap();
    for text in DEFS {
        let mods = eval(vm, &format!("'{text}"));
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
    let odef = vm.heap_mut().wrap(OutputDef::new(scope));
    // The properties' types, which define-context-properties.scm and
    // define-grob-properties.scm give LilyPond's.
    eval(
        vm,
        "(for-each (lambda (s) (set-object-property! s 'translation-type? (lambda (x) #t)))
                   '(measureLength timing tonic fontSize))
         (for-each (lambda (s) (set-object-property! s 'backend-type? (lambda (x) #t)))
                   '(color details length))",
    );
    let t = Translation::new(vm, odef).unwrap().unwrap();
    vm.heap_mut().root(t.value);
    let global = t.arena.borrow().handle(GLOBAL);
    vm.define("test-global", global);
    eval(
        vm,
        "(define log '())
         (ly:add-listener
           (lambda (ev)
             (set! log (cons (cons (car (ly:event-property ev 'class))
                                   (if (ly:event-property ev 'context #f)
                                       (ly:context-name (ly:event-property ev 'context))
                                       (ly:event-property ev 'type)))
                             log)))
           (ly:context-events-below test-global) 'CreateContext 'AnnounceNewContext)",
    );
    t
}

/// The events `log` holds, oldest first, and empties it.
fn log(vm: &mut Vm) -> String {
    let v = eval(vm, "(let ((l (reverse log))) (set! log '()) l)");
    write(vm, v)
}

/// The tree below `c`: `Name=id(children)`.
fn tree(vm: &Vm, a: &Arena, c: CtxId) -> String {
    let mut out = vm.heap().name(a.name(vm.heap(), c)).unwrap().to_string();
    if !a.id(c).is_empty() {
        out.push('=');
        out.push_str(a.id(c));
    }
    let children: Vec<String> = a.children(c).iter().map(|&k| tree(vm, a, k)).collect();
    if !children.is_empty() {
        out.push_str(&format!("({})", children.join(" ")));
    }
    out
}

fn sym(vm: &mut Vm, name: &str) -> Value {
    vm.heap_mut().intern(name)
}

#[test]
fn the_default_bottom_is_made_with_new_ids_as_lilypond_makes_t14s() {
    let mut vm = vm();
    let t = translation(&mut vm);
    // A score's music starts at Global and descends to a bottom context
    // (T14's trace: Score#0=\new, Staff#0=\new, Voice#0).
    let voice = get_default_interpreter(&mut vm, &t, GLOBAL, "").unwrap();
    assert_eq!(
        tree(&vm, &t.arena.borrow(), GLOBAL),
        "Global(Score=\\new(Staff=\\new(Voice)))"
    );
    assert!(t.arena.borrow().is_bottom(voice));
    // The parent's own listener makes the context and announces it before
    // its events below hear the CreateContext, as in LilyPond.
    assert_eq!(
        log(&mut vm),
        "((AnnounceNewContext . Score) (CreateContext . Score) (AnnounceNewContext . Staff) \
         (CreateContext . Staff) (AnnounceNewContext . Voice) (CreateContext . Voice))"
    );
    // A bottom context is its own default interpreter.
    assert_eq!(
        get_default_interpreter(&mut vm, &t, voice, "").unwrap(),
        voice
    );
}

#[test]
fn a_search_finds_before_it_makes_and_reuses_a_score_that_adopts() {
    let mut vm = vm();
    let t = translation(&mut vm);
    let score = get_user_accessible_interpreter(&mut vm, &t, GLOBAL)
        .unwrap()
        .unwrap();
    assert_eq!(tree(&vm, &t.arena.borrow(), GLOBAL), "Global(Score)");
    let (voice_name, staff_name) = (sym(&mut vm, "Voice"), sym(&mut vm, "Staff"));
    // \context Voice = one: Staff made on the way, with no id.
    let one = find_create_context(
        &mut vm,
        &t,
        score,
        Dir::Center,
        voice_name,
        "one",
        Value::NULL,
    )
    .unwrap()
    .unwrap();
    // Found again, from anywhere in the tree.
    assert_eq!(
        find_create_context(
            &mut vm,
            &t,
            GLOBAL,
            Dir::Center,
            voice_name,
            "one",
            Value::NULL
        )
        .unwrap(),
        Some(one)
    );
    // \new Staff from Global: the Score adopts it, and none is made.
    let staff = create_unique_context(
        &mut vm,
        &t,
        GLOBAL,
        Dir::Center,
        staff_name,
        "",
        Value::NULL,
    )
    .unwrap()
    .unwrap();
    assert_eq!(t.arena.borrow().parent(staff), Some(score));
    // \new Voice from the Score: an intermediate Staff named \new.
    create_unique_context(&mut vm, &t, score, Dir::Center, voice_name, "", Value::NULL).unwrap();
    assert_eq!(
        tree(&vm, &t.arena.borrow(), GLOBAL),
        "Global(Score(Staff(Voice=one) Staff Staff=\\new(Voice)))"
    );
    // Up from the voice: its Staff; an alias names it; Bottom is a voice.
    let found = t
        .arena
        .borrow()
        .find_context(vm.heap(), one, Dir::Up, staff_name, "");
    assert_eq!(found, t.arena.borrow().parent(one));
    let alias = sym(&mut vm, "Staff-voice");
    assert!(t.arena.borrow().is_alias(vm.heap(), one, alias));
    vm.define("one", t.arena.borrow().handle(one));
    let v = eval(
        &mut vm,
        "(list (ly:context-name (ly:context-find one 'Score)) (ly:context-find one 'Lyrics) \
                           (ly:context-alias? one 'Bottom) (ly:context-id one) (ly:context-name (ly:context-parent one)) \
                           (length (ly:context-children (ly:context-find one 'Score))))",
    );
    assert_eq!(write(&vm, v), "(Score #f #t \"one\" Staff 3)");
    // A context of no definition is found nowhere and made nowhere.
    let nothing = sym(&mut vm, "Nothing");
    assert_eq!(
        find_create_context(&mut vm, &t, score, Dir::Center, nothing, "", Value::NULL).unwrap(),
        None
    );
}

#[test]
fn properties_come_from_definitions_and_are_checked_when_set() {
    let mut vm = vm();
    let t = translation(&mut vm);
    let voice = get_default_interpreter(&mut vm, &t, GLOBAL, "").unwrap();
    vm.define("voice", t.arena.borrow().handle(voice));
    let v = eval(
        &mut vm,
        "(list (ly:context-property voice 'measureLength)
                                 (ly:context-name (ly:context-property-where-defined voice 'tonic))
                                 (ly:context-property voice 'nothing 'alt)
                                 (ly:context-property voice 'nothing #:default 'none)
                                 (ly:context-property voice 'tonic #:search-ancestors? #f))",
    );
    assert_eq!(write(&vm, v), "(1 Staff alt none ())");
    // A property with a type: an invalid value is warned of and ignored;
    // one with none does not exist.
    eval(
        &mut vm,
        "(define (type-name p) \"number\")
                   (set-object-property! 'fontSize 'translation-type? number?)
                   (ly:context-set-property! voice 'fontSize 3)
                   (ly:context-set-property! voice 'fontSize \"big\")
                   (set! (ly:context-property voice 'nothing) 1)",
    );
    let v = eval(
        &mut vm,
        "(list (ly:context-property voice 'fontSize)
                                 (map (lambda (d) (vector-ref d 2)) (reverse (car %diagnostics))))",
    );
    assert_eq!(
        write(&vm, v),
        "(3 (\"the property 'fontSize' must be of type 'number', ignoring invalid value '\\\"big\\\"'\" \
         \"the property 'nothing' does not exist (perhaps a typing error)\"))"
    );
    // \once \set: undone when the timestep's finalizations run.
    let (font, three, five) = (
        sym(&mut vm, "fontSize"),
        Value::fixnum(3).unwrap(),
        Value::fixnum(5).unwrap(),
    );
    let ev = |vm: &mut Vm, class: &str, value| {
        send(
            vm,
            &t,
            voice,
            class,
            &[("symbol", font), ("value", value), ("once", Value::TRUE)],
        )
        .unwrap()
    };
    ev(&mut vm, "SetProperty", five);
    assert_eq!(t.arena.borrow().here(voice, font), Some(five));
    t.apply_finalizations(&mut vm).unwrap();
    assert_eq!(t.arena.borrow().here(voice, font), Some(three));
    ev(&mut vm, "UnsetProperty", Value::NULL);
    assert_eq!(t.arena.borrow().here(voice, font), None);
    t.apply_finalizations(&mut vm).unwrap();
    assert_eq!(t.arena.borrow().here(voice, font), Some(three));
}

#[test]
fn overrides_stack_on_the_parents_and_once_pops_its_own() {
    let mut vm = vm();
    let t = translation(&mut vm);
    let voice = get_default_interpreter(&mut vm, &t, GLOBAL, "").unwrap();
    vm.define("voice", t.arena.borrow().handle(voice));
    vm.define(
        "staff",
        t.arena
            .borrow()
            .handle(t.arena.borrow().parent(voice).unwrap()),
    );
    // Staff's definition pushed its colour onto Global's description.
    let v = eval(&mut vm, "(list (ly:context-grob-definition voice 'NoteHead) (ly:context-grob-definition voice 'Stem)
                                 (ly:grob-properties? (ly:context-property test-global 'NoteHead)))");
    assert_eq!(
        write(&vm, v),
        "(((color . blue) (color . red) (details (a . 1))) ((length . 7)) #t)"
    );
    // A nested override in the voice, then the staff's own reverted below it.
    eval(
        &mut vm,
        "(ly:context-pushpop-property voice 'NoteHead 'color 'green)
                   (ly:context-pushpop-property staff 'NoteHead 'color)",
    );
    let v = eval(&mut vm, "(ly:context-grob-definition voice 'NoteHead)");
    assert_eq!(
        write(&vm, v),
        "((color . green) (color . red) (details (a . 1)))"
    );
    // \once \override, then its matched pop.
    let (note_head, color) = (sym(&mut vm, "NoteHead"), sym(&mut vm, "color"));
    let path = vm.heap_mut().list(&[color]);
    let yellow = sym(&mut vm, "yellow");
    let cell =
        lily_engine_music::temporary_override(&mut vm, &t, voice, note_head, path, yellow).unwrap();
    vm.define("cell", cell);
    let v = eval(
        &mut vm,
        "(let ((once (ly:context-grob-definition voice 'NoteHead)))
                             (ly:context-matched-pop-property voice 'NoteHead cell)
                             (list once (ly:context-grob-definition voice 'NoteHead)))",
    );
    assert_eq!(
        write(&vm, v),
        "(((color . yellow) (color . green) (color . red) (details (a . 1))) \
         ((color . green) (color . red) (details (a . 1))))"
    );
    // A nested path is folded into its property.
    let (details, b) = (sym(&mut vm, "details"), sym(&mut vm, "b"));
    let path = vm.heap_mut().list(&[details, b]);
    lily_engine_music::push(
        &mut vm,
        &t,
        voice,
        note_head,
        path,
        Value::fixnum(2).unwrap(),
    )
    .unwrap();
    let v = eval(
        &mut vm,
        "(assq 'details (ly:context-grob-definition voice 'NoteHead))",
    );
    assert_eq!(write(&vm, v), "(details (b . 2) (a . 1))");
    // Reverting all the voice's: it holds NoteHead no more.
    eval(
        &mut vm,
        "(ly:context-pushpop-property voice 'NoteHead 'color)",
    );
    let path = vm.heap_mut().list(&[details, b]);
    lily_engine_music::pop(
        vm.heap_mut(),
        &mut t.arena.borrow_mut(),
        voice,
        note_head,
        path,
    );
    assert_eq!(t.arena.borrow().here(voice, note_head), None);
}

#[test]
fn removal_takes_the_deepest_first_and_spares_the_score() {
    let mut vm = vm();
    let t = translation(&mut vm);
    let voice = get_default_interpreter(&mut vm, &t, GLOBAL, "").unwrap();
    let staff = t.arena.borrow().parent(voice).unwrap();
    eval(&mut vm, "(ly:add-listener (lambda (ev) (set! log (cons 'removed log)))
                     (ly:context-event-source (car (ly:context-children (car (ly:context-children test-global))))) 'RemoveContext)");
    t.arena.borrow_mut().set_clients(voice, 1);
    check_removal(&mut vm, &t, GLOBAL).unwrap();
    assert_eq!(
        tree(&vm, &t.arena.borrow(), GLOBAL),
        "Global(Score=\\new(Staff=\\new(Voice)))"
    );
    t.arena.borrow_mut().set_clients(voice, 0);
    log(&mut vm);
    check_removal(&mut vm, &t, GLOBAL).unwrap();
    assert_eq!(tree(&vm, &t.arena.borrow(), GLOBAL), "Global(Score=\\new)");
    assert!(t.arena.borrow().removed(voice) && t.arena.borrow().removed(staff));
    // The Staff's listener heard its own removal only.
    assert_eq!(log(&mut vm), "(removed)");
}

#[test]
fn event_classes_are_sets_of_their_ancestors() {
    let mut vm = vm();
    let [note, rhythmic, music, stream, percent] = [
        "note-event",
        "rhythmic-event",
        "music-event",
        "StreamEvent",
        "percent-event",
    ]
    .map(|n| class_index(n).unwrap());
    let mut classes = EventClasses::new(vm.heap_mut());
    let set = classes.set(note);
    assert!(
        set.contains(note) && set.contains(rhythmic) && set.contains(music) && set.contains(stream)
    );
    assert!(!set.contains(class_index("rest-event").unwrap()));
    // Named twice in the file, kept where it is first: below music-event.
    assert!(!classes.set(percent).contains(rhythmic));
    // A class defined later, by its list.
    let list = eval(
        &mut vm,
        "'(my-event note-event melodic-event rhythmic-event music-event StreamEvent)",
    );
    let mine = classes.class(vm.heap(), list).unwrap();
    let melodic = ClassSet([0; 4]);
    assert!(
        classes.set(mine).contains(note)
            && classes
                .set(mine)
                .contains(class_index("melodic-event").unwrap())
    );
    assert!(
        !melodic.intersects(&classes.set(mine)) && classes.set(note).intersects(&classes.set(mine))
    );
    assert_eq!(classes.class(vm.heap(), list), Some(mine));
}

#[test]
fn listeners_hear_in_the_order_they_were_added() {
    let mut vm = vm();
    let t = translation(&mut vm);
    let voice = get_default_interpreter(&mut vm, &t, GLOBAL, "").unwrap();
    vm.define("voice", t.arena.borrow().handle(voice));
    eval(&mut vm, "(define heard '())
                   (define (hear tag) (lambda (ev) (set! heard (cons tag heard))))
                   (ly:add-listener (hear 'staff-rhythmic) (ly:context-events-below (ly:context-parent voice)) 'rhythmic-event)
                   (ly:add-listener (hear 'voice-music) (ly:context-event-source voice) 'music-event)
                   (ly:add-listener (hear 'voice-note) (ly:context-event-source voice) 'note-event 'rest-event)
                   (ly:add-listener (hear 'score-stream) (ly:context-events-below (ly:context-find voice 'Score)) 'StreamEvent)");
    lily_engine_music::send(&mut vm, &t, voice, "note-event", &[]).unwrap();
    lily_engine_music::send(&mut vm, &t, voice, "bar-event", &[]).unwrap();
    // By priority: the voice's events below were connected to its source
    // first, and the Score's events below to the staff's before the
    // staff's listener was added.
    let v = eval(&mut vm, "(list (reverse heard)
                                 (ly:listened-event-class? (ly:context-event-source voice) '(rest-event rhythmic-event))
                                 (ly:listened-event-class? (ly:context-event-source voice) '(skip-event))
                                 (and (memq 'rest-event (ly:listened-event-types (ly:context-event-source voice))) #t))");
    assert_eq!(
        write(&vm, v),
        "((score-stream staff-rhythmic voice-music voice-note score-stream voice-music) #t #f #t)"
    );
}
