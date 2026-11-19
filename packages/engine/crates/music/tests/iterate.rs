//! N4 (MS2 steps 23–24, 04 §8): music iterated through the global loop on
//! hand-made definitions of LilyPond's shape (Global with a grob, an
//! adopting Score, Staff and Voice as engraver groups), in a VM with only
//! the music crate and the music types: the contexts the iterators find and
//! make, the events they broadcast and when, in `trace.ly`'s format, and
//! the properties at the bar starts `Timing_translator` finds; removal when an iterator lets
//! its context go; frozen music read without thawing; a sequence's elements
//! from a callback of Scheme; moments asked for; a score of no length; the
//! property operations with `\once`, `\temporary` and `pop-first`;
//! `\applyContext` and what it broadcasts; tuplets and their splits; grace
//! notes, their `GraceChange` and `Grace_engraver`'s `graceSettings` (MS2
//! step 35); `\change`, which moves a voice to another staff, and its scope
//! by `change-tag` (step 36); volta repeats and their alternatives, their
//! events, the properties restored after an alternative and
//! `Repeat_acknowledge_engraver`'s `repeatCommands` (step 37), percent
//! repeats' events and `\fine` (MS5 step 6); lyrics
//! combined with their voice (step 38), and the melismas that hold their
//! syllables back, set by the slur, tie and beam engravers and performers
//! (step 39).
//! `tools/compare/test/trace.test.mjs` checks the same against
//! LilyPond on the corpus. The types' table comes from the installed
//! LilyPond 2.26.0; without it these tests are skipped, and nothing is
//! verified.
use std::path::PathBuf;
use std::process::Command;

use lily_engine_music::{
    GLOBAL, Input, Iteration, Moment, OutputDef, Rational, TYPES, Trace, Translation, iterate,
    make_context_def, run_performance, run_translator,
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

fn eval(vm: &mut Vm, text: &str) -> Value {
    vm.eval_text(text, None)
        .unwrap_or_else(|e| panic!("{text}: {}", e.text(vm.heap())))
}

/// The definitions, each a list of mods.
const DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions ((NoteHead . ((color . red))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (default-child Staff) (assign timing #t))",
    "((context-name Staff) (translator-type Engraver_group) (accepts Voice) (default-child Voice))",
    "((context-name Voice) (translator-type Engraver_group))",
];

/// A VM with the music types, a few makers of music, and the layout
/// `layout` of [`DEFS`]; None without the types' table.
fn vm() -> Option<(Vm, Value)> {
    if TYPES.is_empty() {
        eprintln!("skipped: no music types without LilyPond 2.26.0");
        return None;
    }
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
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
    eval(
        &mut vm,
        "(define (note step log . arts)
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 step 0) 'duration (ly:make-duration log)
                       'articulations arts))
         (define (seq . elements) (make-music 'SequentialMusic 'elements elements))
         (define (sim . elements) (make-music 'SimultaneousMusic 'elements elements))
         (define (new type music)
           (make-music 'ContextSpeccedMusic 'context-type type 'create-new #t 'element music))
         (define (staccato) (make-music 'ArticulationEvent 'articulation-type 'staccato))
         (define (in type music) (make-music 'ContextSpeccedMusic 'context-type type 'element music))
         (define (apply-context proc) (make-music 'ApplyContext 'procedure proc))
         (define-public (make-span-event type span-dir) (make-music type 'span-direction span-dir))
         (define-public all-translation-properties '(timing fontSize tupletSpannerDuration))
         (set-object-property! 'timing 'translation-type? boolean?)
         (for-each (lambda (p) (set-object-property! p 'translation-type? number?))
                   '(fontSize tupletSpannerDuration middleCClefPosition middleCOffset))
         (set-object-property! 'NoteHead 'is-grob? #t)
         (set-object-property! 'color 'backend-type? symbol?)",
    );
    let scope = vm.make_module(&["lily"]).unwrap();
    let layout = vm.heap_mut().wrap(OutputDef::new(scope));
    vm.heap_mut().root(layout);
    for text in DEFS {
        define(&mut vm, layout, text);
    }
    Some((vm, layout))
}

/// The definition of the mods `text` defined in `layout`'s scope.
fn define(vm: &mut Vm, layout: Value, text: &str) {
    let scope = vm.heap().foreign::<OutputDef>(layout).unwrap().scope;
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

/// [`DEFS`]' Score with `Timing_translator`, a bar a quarter long, and the
/// types of what it sets: without `time-signature-settings.scm`, beaming
/// is `'()`.
fn timed(vm: &mut Vm, layout: Value) {
    eval(
        vm,
        "(set-object-property! 'measureLength 'translation-type? rational?)
         (set-object-property! 'timeSignature 'translation-type? pair?)
         (set-object-property! 'timeSignatureSettings 'translation-type? list?)",
    );
    define(
        vm,
        layout,
        "((context-name Score) (translator-type Score_engraver) (accepts Staff)
          (default-child Staff) (assign timing #t) (consists Timing_translator)
          (assign measureLength 1/4) (assign timeSignature (1 . 4))
          (assign timeSignatureSettings ((1 . 4))))",
    );
}

/// The trace of `music`'s translation, and the iterators it lacked.
fn traced(vm: &mut Vm, layout: Value, music: &str) -> (String, Vec<&'static str>) {
    let music = eval(vm, music);
    let mut trace = Trace::default();
    let done = run_translator(vm, music, layout, Some(&mut trace))
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    (trace.text, done.unported)
}

/// What the warnings kept in `%diagnostics`, the oldest first.
fn diagnostics(vm: &mut Vm) -> String {
    let v = eval(
        vm,
        "(let ((l (reverse (car %diagnostics)))) (set-car! %diagnostics '())
           (map (lambda (d) (vector-ref d 2)) l))",
    );
    let mut out = String::new();
    vm.heap().write(v, &mut out);
    out
}

#[test]
fn a_sequence_makes_its_contexts_and_reports_events_by_timestep() {
    let Some((mut vm, layout)) = vm() else { return };
    // T14's music: a note, a bar line with no length, a note.
    let (trace, unported) = traced(
        &mut vm,
        layout,
        "(seq (note 1 2) (make-music 'BarEvent 'bar-type \".|:\") (note 1 2))",
    );
    assert_eq!(
        trace,
        "P\t1\tlayout
C\t0\t0\tScore#0=\\new\tGlobal#0
C\t0\t0\tStaff#0=\\new\tScore#0=\\new
C\t0\t0\tVoice#0\tStaff#0=\\new
E\t0\t0\tVoice#0\tnote-event\t#<Pitch d' > #<Duration 4 > () () () () () () ()
E\t1/4\t0\tVoice#0\tbar-event\t() () () () () () () () ()
E\t1/4\t0\tVoice#0\tnote-event\t#<Pitch d' > #<Duration 4 > () () () () () () ()
X\t1/2\t0\tVoice#0
X\t1/2\t0\tStaff#0=\\new
X\t1/2\t0\tScore#0=\\new
"
    );
    assert!(unported.is_empty());
}

#[test]
fn simultaneous_staves_share_the_score_and_end_apart() {
    let Some((mut vm, layout)) = vm() else { return };
    // The second staff ends first: no iterator holds its contexts after
    // 1/4, so they are removed then, the deepest first.
    let (trace, _) = traced(
        &mut vm,
        layout,
        "(sim (new 'Staff (seq (note 0 2) (note 0 2))) (new 'Staff (seq (note 2 2))))",
    );
    assert_eq!(
        trace,
        "P\t1\tlayout
C\t0\t0\tScore#0=\\new\tGlobal#0
C\t0\t0\tStaff#0\tScore#0=\\new
C\t0\t0\tVoice#0\tStaff#0
C\t0\t0\tStaff#1\tScore#0=\\new
C\t0\t0\tVoice#1\tStaff#1
E\t0\t0\tVoice#0\tnote-event\t#<Pitch c' > #<Duration 4 > () () () () () () ()
E\t0\t0\tVoice#1\tnote-event\t#<Pitch e' > #<Duration 4 > () () () () () () ()
E\t1/4\t0\tVoice#0\tnote-event\t#<Pitch c' > #<Duration 4 > () () () () () () ()
X\t1/4\t0\tVoice#1
X\t1/4\t0\tStaff#1
X\t1/2\t0\tVoice#0
X\t1/2\t0\tStaff#0
X\t1/2\t0\tScore#0=\\new
"
    );
}

#[test]
fn labels_and_passes_count_on_across_translations() {
    let Some((mut vm, layout)) = vm() else { return };
    let mut trace = Trace::default();
    for music in ["(seq (note 1 2))", "(new 'Staff (seq (note 1 2)))"] {
        let music = eval(&mut vm, music);
        run_translator(&mut vm, music, layout, Some(&mut trace))
            .unwrap()
            .unwrap();
    }
    let second: Vec<&str> = trace
        .text
        .lines()
        .skip_while(|l| *l != "P\t2\tlayout")
        .collect();
    assert_eq!(
        second[..4],
        [
            "P\t2\tlayout",
            "C\t0\t0\tScore#1=\\new\tGlobal#1",
            "C\t0\t0\tStaff#1\tScore#1=\\new",
            "C\t0\t0\tVoice#1\tStaff#1",
        ]
    );
}

#[test]
fn chords_and_articulations_are_events_of_their_own() {
    let Some((mut vm, layout)) = vm() else { return };
    // A rhythmic event's articulations that a context listens to are
    // broadcast after it; a chord reports its elements, then its
    // articulations.
    let (trace, _) = traced(
        &mut vm,
        layout,
        "(seq (note 0 2 (staccato))
              (make-music 'EventChord 'elements (list (note 0 2) (note 2 2))
                          'articulations (list (staccato))))",
    );
    let events: Vec<&str> = trace.lines().filter(|l| l.starts_with('E')).collect();
    assert_eq!(
        events,
        [
            "E\t0\t0\tVoice#0\tnote-event\t#<Pitch c' > #<Duration 4 > () () () () () () ()",
            "E\t0\t0\tVoice#0\tarticulation-event\t() () () () () staccato () () ()",
            "E\t1/4\t0\tVoice#0\tnote-event\t#<Pitch c' > #<Duration 4 > () () () () () () ()",
            "E\t1/4\t0\tVoice#0\tnote-event\t#<Pitch e' > #<Duration 4 > () () () () () () ()",
            "E\t1/4\t0\tVoice#0\tarticulation-event\t() () () () () staccato () () ()",
        ]
    );
}

#[test]
fn frozen_music_is_iterated_where_it_is() {
    let Some((mut vm, layout)) = vm() else { return };
    let live = "(seq (note 1 2) (new 'Voice (seq (note 3 3) (note 4 3))) (note 1 1))";
    let (expected, _) = traced(&mut vm, layout, live);
    eval(
        &mut vm,
        &format!("(define frozen {live}) (%music-freeze! frozen)"),
    );
    let (trace, _) = traced(&mut vm, layout, "frozen");
    assert_eq!(trace, expected);
    // The events read the nodes, which nothing thawed.
    assert_eq!(eval(&mut vm, "(%music-frozen? frozen)"), Value::TRUE);
}

#[test]
fn a_sequence_takes_its_elements_from_its_callback() {
    let Some((mut vm, layout)) = vm() else { return };
    let (trace, _) = traced(
        &mut vm,
        layout,
        "(let* ((elements (list (note 0 2) (note 4 2)))
                (m (make-music 'SequentialMusic 'elements elements)))
           (ly:music-set-property! m 'elements-callback (lambda (m) (list-copy elements)))
           m)",
    );
    let events: Vec<&str> = trace.lines().filter(|l| l.starts_with('E')).collect();
    assert_eq!(events.len(), 2, "{trace}");
    assert!(events[1].starts_with("E\t1/4\t0\tVoice#0\tnote-event\t#<Pitch g' >"));
    assert_eq!(diagnostics(&mut vm), "()");
}

#[test]
fn music_of_later_iterators_keeps_its_time_and_is_named() {
    let Some((mut vm, layout)) = vm() else { return };
    let (trace, unported) = traced(
        &mut vm,
        layout,
        // A constructor of Scheme's on a sequence of a quarter: none of
        // LilyPond's is left unported since MS5 step 6a.
        "(seq (note 0 2)
              (make-music 'SequentialMusic 'elements (list (note 1 2))
                          'iterator-ctor (lambda () #f))
              (note 2 2))",
    );
    assert_eq!(unported, ["a procedure"]);
    assert!(trace.contains("E\t1/2\t0\tVoice#0\tnote-event\t#<Pitch e' >"));
}

/// The lines of `trace` of the kinds `kinds` (`E`, `S`, …).
fn lines<'a>(trace: &'a str, kinds: &str) -> Vec<&'a str> {
    trace.lines().filter(|l| kinds.contains(&l[..1])).collect()
}

#[test]
fn property_operations_are_events_their_context_hears() {
    let Some((mut vm, layout)) = vm() else { return };
    // \set, \once \set and \unset in the staff, the properties written at
    // each quarter, where Timing_translator starts a bar: the \once is
    // undone after its timestep.
    timed(&mut vm, layout);
    let (trace, unported) = traced(
        &mut vm,
        layout,
        "(seq (in 'Staff (make-music 'PropertySet 'symbol 'fontSize 'value 1)) (note 0 2)
              (in 'Staff (make-music 'PropertySet 'symbol 'fontSize 'value 2 'once #t)) (note 0 2)
              (note 0 2)
              (in 'Staff (make-music 'PropertyUnset 'symbol 'fontSize)) (note 0 2))",
    );
    assert!(unported.is_empty());
    assert_eq!(
        lines(&trace, "ES")
            .into_iter()
            .filter(|l| !l.contains("note-event") && !l.contains("timing"))
            .collect::<Vec<_>>(),
        [
            "E\t0\t0\tStaff#0\tSetProperty\tfontSize 1 ()",
            "S\t0\t0\tStaff#0\tfontSize\t1",
            "E\t1/4\t0\tStaff#0\tSetProperty\tfontSize 2 #t",
            "S\t1/4\t0\tStaff#0\tfontSize\t2",
            "S\t1/2\t0\tStaff#0\tfontSize\t1",
            "E\t3/4\t0\tStaff#0\tUnsetProperty\tfontSize () ()",
        ]
    );
    // The Score's own, from its definition, at each bar: the last at 1,
    // where the music ends, a timestep Timing_translator asks for.
    assert_eq!(
        lines(&trace, "S")
            .iter()
            .filter(|l| l.contains("\ttiming\t#t"))
            .count(),
        5
    );
}

#[test]
fn overrides_push_and_pop_with_once_temporary_and_pop_first() {
    let Some((mut vm, layout)) = vm() else { return };
    // The voice's NoteHead color at each step, read by \applyContext.
    eval(
        &mut vm,
        "(define colors '())
         (define (color) (in 'Voice (apply-context (lambda (c)
           (set! colors (cons (assq-ref (ly:context-grob-definition c 'NoteHead) 'color) colors))))))
         (define (override value . props)
           (in 'Voice (apply make-music 'OverrideProperty 'symbol 'NoteHead
                             'grob-property-path '(color) 'grob-value value props)))
         (define (revert . props)
           (in 'Voice (apply make-music 'RevertProperty 'symbol 'NoteHead
                             'grob-property-path '(color) props)))",
    );
    let (trace, _) = traced(
        &mut vm,
        layout,
        "(seq (note 0 2) (color)
              (override 'blue 'pop-first #t) (override 'green 'pop-first #t) (color)
              (override 'cyan 'once #t) (color) (note 0 2)
              (color) (override 'black) (color) (revert) (color) (revert) (color)
              (revert 'once #t) (override 'white 'pop-first #t) (revert 'once #t) (color) (note 0 2)
              (color) (in 'Voice (make-music 'OverrideProperty 'symbol 'Stem)) (note 0 2))",
    );
    // An \override with pop-first replaces the one before it, \temporary
    // (no pop-first) goes on top and is reverted alone; a \once is popped
    // after its timestep, and a \once \revert, which hides white for its
    // timestep, too.
    assert_eq!(
        eval(
            &mut vm,
            "(equal? (reverse colors) '(red green cyan green black green red red white))"
        ),
        Value::TRUE,
        "{}",
        {
            let v = eval(&mut vm, "(reverse colors)");
            let mut out = String::new();
            vm.heap().write(v, &mut out);
            out
        }
    );
    let events = lines(&trace, "E");
    let voice: Vec<&str> = events
        .iter()
        .filter(|l| !l.contains("note-event"))
        .map(|l| l.split_once("Voice#0\t").unwrap().1)
        .collect();
    assert_eq!(
        voice[..4],
        [
            "Revert\tNoteHead (color) () ()",
            "Override\tNoteHead (color) blue ()",
            "Revert\tNoteHead (color) () ()",
            "Override\tNoteHead (color) green ()",
        ]
    );
    assert_eq!(voice[4], "Override\tNoteHead (color) cyan #t");
    // Stem is no grob here: a warning, and no event.
    assert_eq!(diagnostics(&mut vm), "(\"not a grob name, `Stem'\")");
    assert_eq!(voice.len(), 12, "{voice:#?}");
}

#[test]
fn apply_context_calls_its_procedure_and_broadcasts_after_it() {
    let Some((mut vm, layout)) = vm() else { return };
    // `\partial`'s procedure: it broadcasts from the Score. The event is
    // sent when the procedure returns to the iterator (D85), so a listener
    // has not heard it before then.
    eval(
        &mut vm,
        "(define heard #f)
         (define heard-in-call 'unset)
         (define (partial c)
           (ly:add-listener (lambda (ev) (set! heard (ly:event-property ev 'class)))
                            (ly:context-event-source c) 'partial-event)
           (ly:broadcast (ly:context-event-source c)
                         (ly:make-stream-event '(partial-event music-event StreamEvent)
                                               (list (cons 'duration (ly:make-duration 2)))))
           (set! heard-in-call heard))
         (define (middle-c c)
           (ly:context-set-property! c 'middleCClefPosition -6)
           (ly:context-set-property! c 'middleCOffset 1)
           (ly:set-middle-C! c)
           (set! middle (ly:context-property c 'middleCPosition)))
         (define middle #f)",
    );
    let (trace, _) = traced(
        &mut vm,
        layout,
        "(seq (in 'Score (apply-context partial)) (in 'Staff (apply-context middle-c))
              (make-music 'ApplyContext 'procedure 3) (note 0 2))",
    );
    assert_eq!(eval(&mut vm, "heard-in-call"), Value::FALSE);
    assert_eq!(
        eval(
            &mut vm,
            "(equal? heard '(partial-event music-event StreamEvent))"
        ),
        Value::TRUE
    );
    assert!(
        trace.contains("E\t0\t0\tScore#0\tpartial-event\t() #<Duration 4 > ()"),
        "{trace}"
    );
    assert_eq!(eval(&mut vm, "middle"), Value::fixnum(-5).unwrap());
    assert_eq!(
        diagnostics(&mut vm),
        "(\"\\\\applycontext argument is not a procedure\")"
    );
}

#[test]
fn tuplets_bracket_their_music_and_split_it() {
    let Some((mut vm, layout)) = vm() else { return };
    eval(
        &mut vm,
        "(define (third step)
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 step 0) 'duration (ly:make-duration 3 0 2/3)))
         (define (tuplet . props)
           (apply make-music 'TimeScaledMusic 'numerator 2 'denominator 3
                  'element (seq (third 0) (third 1) (third 2)) props))",
    );
    let span = |trace: &str| -> Vec<String> {
        lines(trace, "E")
            .iter()
            .filter(|l| l.contains("tuplet-span-event"))
            .map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                format!("{} {}", f[1], f[5].split(' ').nth(4).unwrap())
            })
            .collect()
    };
    // A bracket starts with the tuplet's first note and stops at its end.
    let (trace, unported) = traced(&mut vm, layout, "(seq (tuplet) (note 0 2))");
    assert!(unported.is_empty());
    assert_eq!(span(&trace), ["0 -1", "1/4 1"]);
    let notes = lines(&trace, "E")
        .iter()
        .filter(|l| l.contains("note-event"))
        .count();
    assert_eq!(notes, 4);
    // With a duration, a bracket every eighth: a timestep at 1/8, where no
    // note starts.
    let (trace, _) = traced(
        &mut vm,
        layout,
        "(seq (tuplet 'duration (ly:make-duration 3)) (note 0 2))",
    );
    assert_eq!(span(&trace), ["0 -1", "1/8 1", "1/8 -1", "1/4 1"]);
    // Or from tupletSpannerDuration, set in the voice before it.
    let (trace, _) = traced(
        &mut vm,
        layout,
        "(seq (in 'Voice (make-music 'PropertySet 'symbol 'tupletSpannerDuration 'value 1/8))
              (tuplet) (note 0 2))",
    );
    assert_eq!(span(&trace), ["0 -1", "1/8 1", "1/8 -1", "1/4 1"]);
}

#[test]
fn a_score_of_no_length_is_skipped() {
    let Some((mut vm, layout)) = vm() else { return };
    let (trace, _) = traced(&mut vm, layout, "(seq)");
    assert_eq!(trace, "");
    assert_eq!(
        diagnostics(&mut vm),
        "(\"skipping zero-duration score\" \"to suppress this, consider adding a spacer rest\")"
    );
}

#[test]
fn the_loop_stops_at_moments_asked_for() {
    let Some((mut vm, layout)) = vm() else { return };
    let t = Translation::new(&mut vm, layout).unwrap().unwrap();
    vm.heap_mut().root(t.value);
    let global = t.arena.borrow().handle(GLOBAL);
    vm.define("test-global", global);
    // Each timestep's moment, from a listener of Global's `Prepare`.
    eval(
        &mut vm,
        "(define moments '())
         (ly:add-listener
           (lambda (ev) (set! moments (cons (ly:event-property ev 'moment) moments)))
           (ly:context-event-source test-global) 'Prepare)
         (ly:context-schedule-moment test-global (ly:make-moment 1/8))",
    );
    let half = Moment::new(Rational::new(3, 8), Rational::ZERO);
    assert!(t.arena.borrow_mut().add_moment(half));
    let music = eval(&mut vm, "(seq (note 0 2) (note 0 2))");
    let mut it = Iteration::new(vm.heap_mut(), &t);
    assert!(iterate(&mut vm, &t, &mut it, music, false).unwrap());
    let moments = eval(&mut vm, "(reverse moments)");
    let mut out = String::new();
    vm.heap().write(moments, &mut out);
    assert_eq!(
        out,
        "(#<Mom 0> #<Mom 1/8> #<Mom 1/4> #<Mom 3/8> #<Mom 1/2>)"
    );
}

#[test]
fn grace_notes_take_time_before_their_main_note() {
    let Some((mut vm, layout)) = vm() else { return };
    // GraceChange as the voice hears it, when it hears it.
    eval(
        &mut vm,
        "(define (grace music) (make-music 'GraceMusic 'element music))
         (define changes '())
         (define (hear-changes)
           (in 'Voice (apply-context (lambda (c)
             (ly:add-listener (lambda (ev) (set! changes (cons (ly:context-current-moment c) changes)))
                              (ly:context-event-source c) 'GraceChange)))))",
    );
    let (trace, unported) = traced(
        &mut vm,
        layout,
        "(seq (hear-changes) (grace (seq (note 1 3) (note 2 3))) (note 0 2)
              (grace (note 3 4)) (note 0 2))",
    );
    assert!(unported.is_empty(), "{unported:?}");
    // The sequence starts in grace time, its context made there; each
    // grace note is a timestep of its own before its main note.
    assert_eq!(
        lines(&trace, "CE")
            .iter()
            .map(|l| l.split('\t').take(5).collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>(),
        [
            "C 0 -1/4 Score#0 Global#0",
            "C 0 -1/4 Staff#0 Score#0",
            "C 0 -1/4 Voice#0 Staff#0",
            "E 0 -1/4 Voice#0 note-event",
            "E 0 -1/8 Voice#0 note-event",
            "E 0 0 Voice#0 note-event",
            "E 1/4 -1/16 Voice#0 note-event",
            "E 1/4 0 Voice#0 note-event",
        ]
    );
    // Into grace time and out of it, each time the grace iterator is
    // processed across the change.
    let changes =
        "(map (lambda (m) (list (ly:moment-main m) (ly:moment-grace m))) (reverse changes))";
    assert_eq!(
        eval(
            &mut vm,
            &format!("(equal? {changes} '((0 -1/4) (0 0) (1/4 -1/16) (1/4 0)))")
        ),
        Value::TRUE,
        "{}",
        {
            let v = eval(&mut vm, changes);
            let mut out = String::new();
            vm.heap().write(v, &mut out);
            out
        }
    );
}

#[test]
fn grace_engraver_pushes_grace_settings_in_grace_time() {
    let Some((mut vm, layout)) = vm() else { return };
    // A voice with Grace_engraver, whose graceSettings color its note
    // heads blue, and name a context it is not in.
    eval(
        &mut vm,
        "(set-object-property! 'graceSettings 'translation-type? list?)
         (define (grace music) (make-music 'GraceMusic 'element music))
         (define colors '())
         (define (color) (in 'Voice (apply-context (lambda (c)
           (set! colors (cons (assq-ref (ly:context-grob-definition c 'NoteHead) 'color) colors))))))",
    );
    define(
        &mut vm,
        layout,
        "((context-name Voice) (translator-type Engraver_group) (consists Grace_engraver)
          (assign graceSettings ((Voice NoteHead color blue) (Lyrics NoteHead color green))))",
    );
    traced(
        &mut vm,
        layout,
        "(seq (color) (grace (seq (color) (note 1 3))) (color) (note 0 2) (color)
              (grace (seq (color) (note 1 3))) (color) (note 0 2) (color))",
    );
    // Made in grace time, the voice starts with them; the grace iterator's
    // GraceChange at the main note pops them before what comes after the
    // grace; the second grace pushes them again only once its own
    // GraceChange is heard, after the \applyContext before it, as
    // LilyPond's does.
    let colors = "(reverse colors)";
    assert_eq!(
        eval(
            &mut vm,
            &format!("(equal? {colors} '(blue blue red red blue red red))")
        ),
        Value::TRUE,
        "{}",
        {
            let v = eval(&mut vm, colors);
            let mut out = String::new();
            vm.heap().write(v, &mut out);
            out
        }
    );
    // Lyrics is above no voice: LilyPond's programming error, at each
    // push.
    assert_eq!(
        diagnostics(&mut vm),
        "(\"cannot find context from graceSettings: Lyrics\" \"cannot find context from graceSettings: Lyrics\")"
    );
}

/// Two staves, `up` and `down`, with `up`'s music and `down`'s note of a
/// whole: the ids of the parents of the contexts `(parent)` was applied in,
/// in order, the trace and the warnings.
fn changed(vm: &mut Vm, layout: Value, up: &str) -> (String, String, String) {
    eval(
        vm,
        "(define (named type id music)
           (make-music 'ContextSpeccedMusic 'context-type type 'context-id id 'create-new #t
                       'element music))
         (define* (change id #:optional (type 'Staff) tag)
           (apply make-music 'ContextChange 'change-to-type type 'change-to-id id
                  (if tag (list 'change-tag tag) '())))
         (define parents '())
         (define (parent c)
           (set! parents (cons (ly:context-id (ly:context-parent c)) parents)))",
    );
    let music = format!("(sim (named 'Staff \"up\" {up}) (named 'Staff \"down\" (note 0 0)))");
    let (trace, unported) = traced(vm, layout, &music);
    assert!(unported.is_empty(), "{unported:?}");
    let mut parents = String::new();
    let list = eval(vm, "(reverse parents)");
    vm.heap().write(list, &mut parents);
    (parents, trace, diagnostics(vm))
}

#[test]
fn change_moves_the_voice_to_the_staff_of_its_id() {
    let Some((mut vm, layout)) = vm() else { return };
    let (parents, trace, warnings) = changed(
        &mut vm,
        layout,
        "(seq (note 0 2) (apply-context parent) (change \"down\") (apply-context parent)
              (note 1 2) (change \"nowhere\") (change \"up\" 'Lyrics) (note 2 2)
              (apply-context parent))",
    );
    // The voice, made below `up`, is below `down` from the change on; the
    // changes that find no context leave it there.
    assert_eq!(parents, "(\"up\" \"down\" \"down\")");
    assert_eq!(
        warnings,
        "(\"cannot find context to change to: Staff = nowhere\" \
         \"cannot find context to change from: Lyrics\")"
    );
    assert_eq!(
        lines(&trace, "C"),
        [
            "C\t0\t0\tScore#0=\\new\tGlobal#0",
            "C\t0\t0\tStaff#0=up\tScore#0=\\new",
            "C\t0\t0\tVoice#0\tStaff#0=up",
            "C\t0\t0\tStaff#1=down\tScore#0=\\new",
            "C\t0\t0\tVoice#1\tStaff#1=down",
        ]
    );
    // Its events are its own still.
    assert!(
        trace.contains("E\t1/2\t0\tVoice#0\tnote-event\t#<Pitch e' >"),
        "{trace}"
    );
}

#[test]
fn a_change_tag_moves_every_voice_of_the_tagged_music() {
    let Some((mut vm, layout)) = vm() else { return };
    // Two voices of `up`: the first changes, the second applies `parent`
    // after it. With the tag on the music of both, both move
    // (`where_tagged`, `preorder_walk`); with none, or one no music has,
    // only the first.
    let both = |tag: &str, tags: &str| {
        format!(
            "(make-music 'SimultaneousMusic 'tags '{tags} 'elements
               (list (new 'Voice (seq (note 0 2) (change \"down\" 'Staff {tag}) (note 1 2)))
                     (new 'Voice (seq (note 0 2) (note 1 2) (apply-context parent) (note 2 2)))))"
        )
    };
    for (tag, tags, expected) in [
        ("'both", "(both)", "(\"down\")"),
        ("#f", "(both)", "(\"up\")"),
        ("'other", "(both)", "(\"up\")"),
    ] {
        let (parents, _, warnings) = changed(&mut vm, layout, &both(tag, tags));
        assert_eq!(parents, expected, "{tag}");
        assert_eq!(warnings, "()");
    }
}

/// A volta repeat of `count` with `body` and the alternatives `alts`, each
/// `(numbers music)`, as `make-repeat` makes it.
fn volta(count: u32, body: &str, alts: &[(&str, &str)]) -> String {
    let alts: Vec<String> = alts
        .iter()
        .map(|(nums, m)| {
            format!("(make-music 'VoltaSpeccedMusic 'volta-numbers '{nums} 'element {m})")
        })
        .collect();
    format!(
        "(make-music 'VoltaRepeatedMusic 'repeat-count {count} 'element {body}
                     'elements (list {}))",
        alts.join(" ")
    )
}

#[test]
fn a_volta_repeat_reports_its_start_alternatives_and_returns() {
    let Some((mut vm, layout)) = vm() else { return };
    timed(&mut vm, layout);
    eval(
        &mut vm,
        "(set-object-property! 'repeatCommands 'translation-type? list?)
         (set! all-translation-properties (cons 'repeatCommands all-translation-properties))",
    );
    define(
        &mut vm,
        layout,
        "((context-name Score) (translator-type Score_engraver) (accepts Staff)
          (default-child Staff) (assign timing #t)
          (consists Timing_translator) (consists Repeat_acknowledge_engraver)
          (assign measureLength 1/4) (assign timeSignature (1 . 4))
          (assign timeSignatureSettings ((1 . 4))))",
    );
    // Three voltas, the first two ending in one alternative, the third in
    // another, then a note. The repeat's context is the voice its body
    // made, whose events the trace writes as the voice's.
    let music = format!(
        "(seq {} (note 0 2))",
        volta(
            3,
            "(seq (note 0 2))",
            &[("(1 2)", "(note 1 2)"), ("(3)", "(note 2 2)")]
        )
    );
    let (trace, unported) = traced(&mut vm, layout, &music);
    assert!(unported.is_empty(), "{unported:?}");
    let events: Vec<String> = lines(&trace, "E")
        .iter()
        .map(|l| l.split('\t').take(5).collect::<Vec<_>>().join(" "))
        .collect();
    assert_eq!(
        events,
        [
            "E 0 0 Voice#0 volta-repeat-start-event",
            "E 0 0 Voice#0 note-event",
            "E 1/4 0 Voice#0 alternative-event",
            "E 1/4 0 Voice#0 volta-span-event",
            "E 1/4 0 Voice#0 note-event",
            "E 1/2 0 Voice#0 volta-span-event",
            "E 1/2 0 Voice#0 volta-repeat-end-event",
            "E 1/2 0 Voice#0 alternative-event",
            "E 1/2 0 Voice#0 volta-span-event",
            "E 1/2 0 Voice#0 note-event",
            "E 3/4 0 Voice#0 volta-span-event",
            "E 3/4 0 Voice#0 alternative-event",
            "E 3/4 0 Voice#0 note-event",
        ],
        "{trace}"
    );
    // The start and the first alternative's two returns, in
    // repeatCommands where it is defined, emptied at the next timestep.
    let commands: Vec<&str> = lines(&trace, "S")
        .into_iter()
        .filter(|l| l.contains("\trepeatCommands\t"))
        .collect();
    assert_eq!(
        commands,
        [
            "S\t0\t0\tScore#0=\\new\trepeatCommands\t((start-repeat 3))",
            "S\t1/4\t0\tScore#0=\\new\trepeatCommands\t()",
            "S\t1/2\t0\tScore#0=\\new\trepeatCommands\t((end-repeat 2))",
            "S\t3/4\t0\tScore#0=\\new\trepeatCommands\t()",
            "S\t1\t0\tScore#0=\\new\trepeatCommands\t()",
        ]
    );
    assert_eq!(diagnostics(&mut vm), "()");
}

#[test]
fn a_percent_repeat_plays_its_body_once_and_reports_each_repetition() {
    let Some((mut vm, layout)) = vm() else { return };
    timed(&mut vm, layout);
    // music-functions.scm's, for a body of main time only.
    eval(
        &mut vm,
        "(define-public (skip-of-moment-span start end)
           (make-music 'SkipMusic
                       'duration (ly:make-duration 0 0 (ly:moment-main (ly:moment-sub end start)))))",
    );
    // Bars of a quarter: a body of one bar is a percent, of two a double
    // percent, of an eighth a repeat slash (MS5 step 6). Each repetition's
    // event comes at its start, the body's notes once; then a `\fine`.
    let percent = |count: u32, body: &str| {
        format!("(make-music 'PercentRepeatedMusic 'repeat-count {count} 'element {body})")
    };
    let music = format!(
        "(seq {} {} {} (make-music 'FineEvent) (note 0 2))",
        percent(3, "(note 0 2)"),
        percent(2, "(seq (note 1 2) (note 2 2))"),
        percent(2, "(note 3 3)"),
    );
    let (trace, unported) = traced(&mut vm, layout, &music);
    assert!(unported.is_empty(), "{unported:?}");
    let events: Vec<String> = lines(&trace, "E")
        .iter()
        .map(|l| l.split('\t').take(5).collect::<Vec<_>>().join(" "))
        .collect();
    assert_eq!(
        events,
        [
            "E 0 0 Voice#0 note-event",
            "E 1/4 0 Voice#0 percent-event",
            "E 1/2 0 Voice#0 percent-event",
            "E 3/4 0 Voice#0 note-event",
            "E 1 0 Voice#0 note-event",
            "E 5/4 0 Voice#0 double-percent-event",
            "E 7/4 0 Voice#0 note-event",
            "E 15/8 0 Voice#0 repeat-slash-event",
            "E 2 0 Voice#0 fine-event",
            "E 2 0 Voice#0 note-event",
        ],
        "{trace}"
    );
    // Music after `\fine` outside a folded repeat.
    assert_eq!(diagnostics(&mut vm), "(\"found music after \\\\fine\")");
}

#[test]
fn alternatives_restore_what_they_save_and_warn_of_a_missing_volta() {
    let Some((mut vm, layout)) = vm() else { return };
    // fontSize, saved where it is defined (the Score) as the first
    // alternative starts, is set again after it, not after the last.
    eval(
        &mut vm,
        "(set-object-property! 'alternativeRestores 'translation-type? list?)
         (define sizes '())
         (define (size) (in 'Voice (apply-context (lambda (c)
           (set! sizes (cons (ly:context-property c 'fontSize) sizes))))))
         (define (grow) (apply-context (lambda (c)
           (ly:context-set-property! (ly:context-find c 'Score) 'fontSize 3))))",
    );
    define(
        &mut vm,
        layout,
        "((context-name Score) (translator-type Score_engraver) (accepts Staff)
          (default-child Staff) (assign timing #t) (assign fontSize 1)
          (assign alternativeRestores (fontSize)))",
    );
    let music = format!(
        "(seq {} (size))",
        volta(
            2,
            "(seq (note 0 2))",
            &[
                ("(1)", "(seq (grow) (size) (note 1 2))"),
                ("(2)", "(seq (size) (grow) (note 2 2))")
            ]
        )
    );
    traced(&mut vm, layout, &music);
    assert_eq!(
        eval(&mut vm, "(equal? (reverse sizes) '(3 1 3))"),
        Value::TRUE
    );
    // An alternative with no volta numbers: LilyPond's warning, and no
    // bracket for it.
    let music = format!(
        "(seq {})",
        volta(2, "(seq (note 0 2))", &[("(1)", "(note 1 2)")]).replace(
            "(make-music 'VoltaSpeccedMusic 'volta-numbers '(1) 'element (note 1 2))",
            "(note 1 2) (note 2 2)"
        )
    );
    let (trace, _) = traced(&mut vm, layout, &music);
    assert_eq!(
        diagnostics(&mut vm),
        "(\"missing volta specification on alternative element\" \
         \"missing volta specification on alternative element\")"
    );
    assert!(!trace.contains("volta-span-event"), "{trace}");
}

/// [`DEFS`]' Score accepting `Lyrics` too, `Lyrics` with
/// `associatedVoiceType` as `engraver-init.ly` gives it, the types of the
/// properties the lyrics read, and makers of syllables,
/// `\lyricsto`, named and `\with`ed contexts, rests, skips and graces.
fn lyrical(vm: &mut Vm, layout: Value) {
    define(
        vm,
        layout,
        "((context-name Score) (translator-type Score_engraver) (accepts Staff)
          (accepts Lyrics) (default-child Staff) (assign timing #t))",
    );
    define(
        vm,
        layout,
        "((context-name Lyrics) (translator-type Engraver_group)
          (assign associatedVoiceType Voice))",
    );
    eval(
        vm,
        "(define (named type id music)
           (make-music 'ContextSpeccedMusic 'context-type type 'context-id id 'create-new #t
                       'element music))
         (define (with type ops music)
           (make-music 'ContextSpeccedMusic 'context-type type 'create-new #t
                       'property-operations ops 'element music))
         (define (syllables . texts)
           (apply seq (map (lambda (t) (make-music 'LyricEvent 'text t
                                                   'duration (ly:make-duration 2)))
                           texts)))
         (define (lyricsto id music)
           (make-music 'LyricCombineMusic 'associated-context id 'element music))
         (define (rest) (make-music 'RestEvent 'duration (ly:make-duration 2)))
         (define (skip log) (make-music 'SkipEvent 'duration (ly:make-duration log)))
         (define (grace music) (make-music 'GraceMusic 'element music))
         (define (set-to sym value) (make-music 'PropertySet 'symbol sym 'value value))
         (for-each (lambda (p) (set-object-property! (car p) 'translation-type? (cdr p)))
                   (list (cons 'associatedVoice string?) (cons 'associatedVoiceType symbol?)
                         (cons 'associatedVoiceContext ly:context?)
                         (cons 'includeGraceNotes boolean?)))
         (set! all-translation-properties
               (append '(associatedVoice associatedVoiceType associatedVoiceContext
                         includeGraceNotes)
                       all-translation-properties))",
    );
}

/// The lyrics' events of `trace`: their moments, contexts and texts.
fn syllables(trace: &str) -> Vec<String> {
    lines(trace, "E")
        .iter()
        .filter(|l| l.contains("\tlyric-event\t"))
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let text = f[5].split('"').nth(1).unwrap_or_default();
            format!("{} {} {} \"{text}\"", f[1], f[2], f[3])
        })
        .collect()
}

#[test]
fn syllables_start_with_the_voices_notes_and_wait_out_rests_and_graces() {
    let Some((mut vm, layout)) = vm() else { return };
    lyrical(&mut vm, layout);
    // A note, a rest, a note, a grace before a note: the syllables go to
    // the notes, the rest gets none, and the grace's waits for its main
    // note unless the lyrics take grace notes; the last is left over.
    let (trace, unported) = traced(
        &mut vm,
        layout,
        "(sim (new 'Staff (named 'Voice \"v\"
                (seq (note 0 2) (rest) (note 1 2) (grace (note 2 3)) (note 3 2))))
              (new 'Lyrics (lyricsto \"v\" (syllables \"a\" \"b\" \"c\" \"d\")))
              (with 'Lyrics '((assign includeGraceNotes #t))
                    (lyricsto \"v\" (syllables \"a\" \"b\" \"c\" \"d\" \"e\"))))",
    );
    assert!(unported.is_empty(), "{unported:?}");
    assert_eq!(
        syllables(&trace),
        [
            "0 0 Lyrics#0 \"a\"",
            "0 0 Lyrics#1 \"a\"",
            "1/2 0 Lyrics#0 \"b\"",
            "1/2 0 Lyrics#1 \"b\"",
            "3/4 -1/8 Lyrics#1 \"c\"",
            "3/4 0 Lyrics#0 \"c\"",
            "3/4 0 Lyrics#1 \"d\"",
        ]
    );
    // The lyrics end with the voice.
    assert_eq!(
        lines(&trace, "X")[..3],
        [
            "X\t1\t0\tVoice#0=v",
            "X\t1\t0\tStaff#0",
            "X\t1\t0\tLyrics#0"
        ]
    );
    assert_eq!(diagnostics(&mut vm), "()");
}

#[test]
fn lyrics_find_a_voice_made_later_and_follow_associated_voice() {
    let Some((mut vm, layout)) = vm() else { return };
    lyrical(&mut vm, layout);
    // `late` is made at 1, when its `CreateContext` finds it for the
    // lyrics, which then switch to `other` after their second syllable.
    // As in LilyPond, a voice iterated after its lyrics would give them no
    // syllable: they look for its note before it is heard.
    let (trace, _) = traced(
        &mut vm,
        layout,
        "(sim (new 'Staff (seq (skip 0) (named 'Voice \"late\" (seq (note 0 2) (note 1 2)))))
              (new 'Staff (named 'Voice \"other\"
                (apply seq (map (lambda (i) (note 0 3)) (iota 16)))))
              (new 'Lyrics (lyricsto \"late\"
                (seq (syllables \"a\") (set-to 'associatedVoice \"other\")
                     (syllables \"b\" \"c\")))))",
    );
    assert_eq!(
        syllables(&trace),
        [
            "1 0 Lyrics#0 \"a\"",
            "5/4 0 Lyrics#0 \"b\"",
            "11/8 0 Lyrics#0 \"c\""
        ]
    );
    assert!(
        lines(&trace, "C").contains(&"C\t1\t0\tVoice#2=late\tStaff#0"),
        "{trace}"
    );
    assert_eq!(diagnostics(&mut vm), "()");
}

#[test]
fn lyrics_without_their_voice_or_lyrics_context_warn() {
    let Some((mut vm, layout)) = vm() else { return };
    lyrical(&mut vm, layout);
    traced(
        &mut vm,
        layout,
        "(sim (new 'Staff (named 'Voice \"v\" (seq (note 0 2) (note 1 2))))
              (new 'Lyrics (lyricsto \"nowhere\" (syllables \"a\" \"b\"))))",
    );
    assert_eq!(
        diagnostics(&mut vm),
        "(\"cannot find context: Voice = nowhere\")"
    );
    // Music with no `Lyrics` context: its syllables go to a context made
    // for them, and the voice is never followed.
    traced(
        &mut vm,
        layout,
        "(sim (new 'Staff (named 'Voice \"v\" (seq (note 0 2) (note 1 2))))
              (lyricsto \"v\" (syllables \"a\" \"b\")))",
    );
    assert_eq!(
        diagnostics(&mut vm),
        "(\"argument of \\\\lyricsto should contain Lyrics context\" \
         \"cannot find context: Voice = v\")"
    );
}

#[test]
fn the_voices_repeats_are_forwarded_to_its_lyrics() {
    let Some((mut vm, layout)) = vm() else { return };
    lyrical(&mut vm, layout);
    // What the lyrics' context hears of the structural events, which the
    // trace writes as the voice's only.
    eval(
        &mut vm,
        "(define heard '())
         (define (listen c)
           (ly:add-listener (lambda (ev) (set! heard (cons (car (ly:event-property ev 'class))
                                                           heard)))
                            (ly:context-event-source c) 'structural-event))",
    );
    let music = format!(
        "(sim (new 'Staff (named 'Voice \"v\" (seq (note 0 2) {})))
              (new 'Lyrics (lyricsto \"v\" (seq (apply-context listen)
                                               (syllables \"a\" \"b\" \"c\" \"d\")))))",
        volta(
            2,
            "(seq (note 1 2))",
            &[("(1)", "(note 2 2)"), ("(2)", "(note 3 2)")]
        )
    );
    let (trace, unported) = traced(&mut vm, layout, &music);
    assert!(unported.is_empty(), "{unported:?}");
    let mut heard = String::new();
    let list = eval(&mut vm, "(reverse heard)");
    vm.heap().write(list, &mut heard);
    assert_eq!(
        heard,
        "(volta-repeat-start-event alternative-event volta-repeat-end-event \
         alternative-event alternative-event)"
    );
    assert_eq!(syllables(&trace).len(), 4);
}

/// [`lyrical`]'s, with the translators that set the melisma properties in
/// `Voice` (`Note_heads_engraver`, a placeholder, so that the tie engraver
/// hears its notes), `melismaBusyProperties` as LilyPond's Score gives
/// it, and makers of slurs, ties and beams.
fn melismatic(vm: &mut Vm, layout: Value) {
    lyrical(vm, layout);
    define(
        vm,
        layout,
        "((context-name Score) (translator-type Score_engraver) (accepts Staff)
          (accepts Lyrics) (default-child Staff) (assign timing #t)
          (assign melismaBusyProperties
                  (melismaBusy slurMelismaBusy tieMelismaBusy beamMelismaBusy completionBusy)))",
    );
    define(
        vm,
        layout,
        "((context-name Voice) (translator-type Engraver_group)
          (consists Note_heads_engraver) (consists Beam_engraver) (consists Grace_beam_engraver)
          (consists Slur_engraver) (consists Tie_engraver))",
    );
    eval(
        vm,
        "(define (slur dir) (make-span-event 'SlurEvent dir))
         (define (beam dir) (make-span-event 'BeamEvent dir))
         (define (tie) (make-music 'TieEvent))
         (define (in-voice id . notes) (new 'Staff (named 'Voice id (apply seq notes))))
         (define (lyrics-to id . texts) (new 'Lyrics (lyricsto id (apply syllables texts))))
         (for-each (lambda (p) (set-object-property! p 'translation-type? boolean?))
                   '(melismaBusy slurMelismaBusy tieMelismaBusy beamMelismaBusy completionBusy
                     autoBeaming ignoreMelismata tieWaitForNote))
         (set-object-property! 'melismaBusyProperties 'translation-type? list?)",
    );
}

/// The moments of the syllables of `trace`'s lyrics context `n`.
fn sung(trace: &str, n: u32) -> Vec<String> {
    syllables(trace)
        .iter()
        .filter(|s| s.contains(&format!(" Lyrics#{n} ")))
        .map(|s| s.split(" Lyrics").next().unwrap().to_string())
        .collect()
}

#[test]
fn slurs_ties_beams_and_melisma_busy_hold_syllables_back() {
    let Some((mut vm, layout)) = vm() else { return };
    melismatic(&mut vm, layout);
    // A slur over two notes; a tie, and one of a chord; a beam with
    // autoBeaming off, then on, where it holds none; \melisma as a
    // property; the same lyrics ignoring melismata.
    let (trace, unported) = traced(
        &mut vm,
        layout,
        "(sim (in-voice \"v\"
                (note 0 2 (slur -1)) (note 1 2 (slur 1)) (note 2 2 (tie)) (note 2 2)
                (make-music 'EventChord 'elements (list (note 0 2 (tie)) (note 2 2)))
                (make-music 'EventChord 'elements (list (note 0 2) (note 2 2)))
                (set-to 'autoBeaming #f)
                (note 3 3 (beam -1)) (note 4 3 (beam 1)) (set-to 'autoBeaming #t)
                (note 3 3 (beam -1)) (note 4 3 (beam 1))
                (set-to 'melismaBusy #t) (note 5 2) (note 6 2)
                (make-music 'PropertyUnset 'symbol 'melismaBusy) (note 0 2))
              (lyrics-to \"v\" \"a\" \"b\" \"c\" \"d\" \"e\" \"f\" \"g\" \"h\" \"i\" \"j\" \"k\")
              (with 'Lyrics '((assign ignoreMelismata #t))
                (lyricsto \"v\" (syllables \"a\" \"b\" \"c\" \"d\" \"e\" \"f\" \"g\" \"h\" \"i\"
                                           \"j\" \"k\" \"l\"))))",
    );
    assert!(unported.is_empty(), "{unported:?}");
    assert_eq!(
        sung(&trace, 0),
        ["0 0", "1/2 0", "1 0", "3/2 0", "7/4 0", "15/8 0", "5/2 0"]
    );
    assert_eq!(sung(&trace, 1).len(), 12);
    assert_eq!(diagnostics(&mut vm), "()");
}

#[test]
fn ties_wait_for_their_notes_as_lilyponds_do() {
    let Some((mut vm, layout)) = vm() else { return };
    melismatic(&mut vm, layout);
    // Enharmonic notes are tied; a tie to another pitch holds the next note
    // back too, till its end has passed; with tieWaitForNote the tie waits
    // for its pitch, holding every note back till then.
    let (trace, _) = traced(
        &mut vm,
        layout,
        "(define (pitched octave step alter log . arts)
           (make-music 'NoteEvent 'pitch (ly:make-pitch octave step alter)
                       'duration (ly:make-duration log) 'articulations arts))
         (sim (in-voice \"v\"
                (pitched 0 0 1/2 2 (tie)) (pitched 0 1 -1/2 2) (note 2 2 (tie)) (note 3 2)
                (note 4 2) (set-to 'tieWaitForNote #t) (note 0 2 (tie)) (note 1 2) (note 0 2)
                (note 2 2))
              (lyrics-to \"v\" \"a\" \"b\" \"c\" \"d\" \"e\" \"f\"))",
    );
    assert_eq!(sung(&trace, 0), ["0 0", "1/2 0", "1 0", "5/4 0", "2 0"]);
}

#[test]
fn a_context_with_children_is_held_only_by_all_of_them() {
    let Some((mut vm, layout)) = vm() else { return };
    melismatic(&mut vm, layout);
    // Lyrics that follow a staff of two voices: a slur in one alone holds
    // no syllable back; in both it does.
    let (trace, _) = traced(
        &mut vm,
        layout,
        "(sim (named 'Staff \"s\"
                (sim (new 'Voice (seq (note 0 2 (slur -1)) (note 1 2 (slur 1)) (note 2 2 (slur -1))
                                      (note 3 2 (slur 1))))
                     (new 'Voice (seq (note 0 2) (note 1 2) (note 2 2 (slur -1))
                                      (note 3 2 (slur 1))))))
              (new 'Lyrics (make-music 'LyricCombineMusic 'associated-context \"s\"
                                       'associated-context-type 'Staff
                                       'element (syllables \"a\" \"b\" \"c\" \"d\"))))",
    );
    assert_eq!(sung(&trace, 0), ["0 0", "1/4 0", "1/2 0"]);
}

#[test]
fn the_performers_set_the_melisma_properties_in_midi() {
    let Some((mut vm, layout)) = vm() else { return };
    melismatic(&mut vm, layout);
    for text in [
        "((context-name Score) (translator-type Score_performer) (accepts Staff)
          (accepts Lyrics) (default-child Staff) (assign timing #t)
          (assign melismaBusyProperties
                  (melismaBusy slurMelismaBusy tieMelismaBusy beamMelismaBusy completionBusy)))",
        "((context-name Staff) (translator-type Performer_group) (accepts Voice)
          (default-child Voice))",
        "((context-name Voice) (translator-type Performer_group) (consists Note_performer)
          (consists Tie_performer) (consists Slur_performer) (consists Beam_performer))",
        "((context-name Lyrics) (translator-type Performer_group)
          (assign associatedVoiceType Voice))",
    ] {
        define(&mut vm, layout, text);
    }
    // A slur, a tie, a beam with autoBeaming off and one with it on, and a
    // staccato tied note, whose shorter end lets the tie go before the
    // next note: its syllable is not held back. The map has the events of
    // an origin: the syllables'.
    let at = vm
        .heap_mut()
        .wrap(Input::new(0, 0, 1, "/a.ly".into(), [1, 0, 0]));
    vm.define("loc", at);
    let music = eval(
        &mut vm,
        "(define-public (staccato-length len context) (/ len 2))
         (define (lyrics-to id . texts)
           (new 'Lyrics (lyricsto id (apply seq (map (lambda (t)
             (make-music 'LyricEvent 'text t 'duration (ly:make-duration 2) 'origin loc))
             texts)))))
         (sim (in-voice \"v\"
                (note 0 2 (slur -1)) (note 1 2 (slur 1)) (note 2 2 (tie)) (note 2 2)
                (set-to 'autoBeaming #f) (note 3 3 (beam -1)) (note 4 3 (beam 1))
                (set-to 'autoBeaming #t) (note 3 3 (beam -1)) (note 4 3 (beam 1))
                (note 5 2 (tie)
                      (make-music 'ArticulationEvent 'articulation-type 'staccato
                                  'midi-length staccato-length))
                (note 5 2))
              (new 'Staff (named 'Voice \"w\" (apply seq (map (lambda (i) (note 0 3)) (iota 16)))))
              (lyrics-to \"v\" \"a\" \"b\" \"c\" \"d\" \"e\" \"f\" \"g\" \"h\"))",
    );
    vm.heap_mut().root(music);
    let done = run_performance(&mut vm, music, layout)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    let map = done.playback().unwrap();
    // The syllables' moments: events of no length.
    let at: Vec<&str> = map
        .split("{\"href\"")
        .filter(|e| e.contains("\"length\":0}"))
        .map(|e| {
            e.split("\"at\":")
                .nth(1)
                .unwrap()
                .split(',')
                .next()
                .unwrap()
        })
        .collect();
    assert_eq!(at, ["0", "0.5", "1", "1.25", "1.375", "1.5", "1.75"]);
}
