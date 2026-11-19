//! N5's first part (MS2 step 25, 04 §9.1, D79): translator groups made
//! from the definitions' `consists`, in LilyPond's order and filtered by
//! output; the timestep protocol, with listeners and acknowledgers that
//! wake translators only when they have work; grob and audio records;
//! `Timing_translator`'s properties, bar numbers and the timesteps it asks
//! for; and the `ly:translator-*` face. Since step 26, translators written
//! in Scheme, run with the same wake rules, the grobs they make and
//! acknowledge, and the playback map a `\midi` translation writes.
//! Hand-made definitions of
//! LilyPond's shape and test classes that log their calls to Scheme, in a
//! VM with only the music crate and the music types; `time-signature-
//! settings.scm`'s functions are small stand-ins here, and the real ones
//! run in `tools/compare/test/trace.test.mjs`, which compares the timing
//! properties with LilyPond's on the corpus. Without LilyPond 2.26.0's
//! music types these tests are skipped, and nothing is verified.
use std::path::PathBuf;
use std::process::Command;

use lily_engine_music::{
    ACKNOWLEDGE_AUDIO, Cause, GLOBAL, OutputDef, PROCESS, PROCESS_ACKNOWLEDGED, START, STOP, TYPES,
    Trace, Translated, TranslatorClass, Tx, Work, event_property, make_context_def,
    run_translator_with, translator_handle,
};
use lily_engine_music::{FINALIZE, INITIALIZE, Input, Kind, PRE_PROCESS, run_performance};
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

/// Global, a Score that keeps time, Staff and Voice, with the test classes
/// and LilyPond's names; a MIDI Staff.
const DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score))",
    "((context-name Score) (translator-type Score_engraver) (accepts Staff)
      (default-child Staff) (assign timing #t) (assign timeSignature (2 . 4))
      (assign timeSignatureSettings ((2 . 4))) (consists Timing_translator)
      (consists Probe_last) (consists Probe_score))",
    "((context-name Staff) (translator-type Engraver_group) (accepts Voice)
      (default-child Voice) (consists Acknowledger) (consists Note_performer)
      (consists Axis_group_engraver) (consists Clef_engraver))",
    "((context-name Voice) (translator-type Engraver_group) (consists Note_maker)
      (consists Probe_voice))",
];

/// A VM with the music types, the makers of music, the stand-ins of
/// `time-signature-settings.scm`, `probe!` logging to `probes`, and the
/// layout of [`DEFS`].
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
        "(define (note step log)
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 step 0) 'duration (ly:make-duration log)))
         (define (rest log) (make-music 'RestEvent 'duration (ly:make-duration log)))
         (define (seq . elements) (make-music 'SequentialMusic 'elements elements))
         (define (sim . elements) (make-music 'SimultaneousMusic 'elements elements))
         (define (new type music)
           (make-music 'ContextSpeccedMusic 'context-type type 'create-new #t 'element music))
         (define (in type music) (make-music 'ContextSpeccedMusic 'context-type type 'element music))
         (define (event type . props) (apply make-music type props))
         (define-public (make-apply-context proc) (make-music 'ApplyContext 'procedure proc))
         (define-public (descend-to-context m type)
           (make-music 'ContextSpeccedMusic 'context-type type 'descend-only #t 'element m))
         ;; A class the table numbers is found by its name.
         (define-public (ly:make-event-class class) (list class))
         (define-public all-translation-properties '(timing))
         (define-public probes '())
         (define-public (probe! . what) (set! probes (cons what probes)))
         (define-public (calc-measure-length ts) (/ (car ts) (cdr ts)))
         (define-public (beam-exceptions ts settings) (list 'exceptions ts))
         (define-public (beat-base ts settings) (/ 1 (cdr ts)))
         (define-public (beat-structure base ts settings) (list 'beats base))
         (define-public (calc-submeasure-structure base ts settings) (list (car ts)))
         (set-object-property! 'timing 'translation-type? boolean?)
         (set-object-property! 'timeSignature 'translation-type? pair?)
         (set-object-property! 'timeSignatureSettings 'translation-type? list?)
         (set-object-property! 'currentBarNumber 'translation-type? integer?)",
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

// --- the test classes ----------------------------------------------------

/// Logs `(name what moment)` to `probes`.
fn log(tx: &mut Tx<'_>, name: &str, what: &str) -> Run<()> {
    let (name, what) = (tx.heap().intern(name), tx.heap().string(what));
    let now = tx.now();
    let now = tx.heap().wrap(now);
    tx.call("probe!", &[name, what, now])?;
    Ok(())
}

/// A translator of all methods that hears notes and runs only when woken;
/// it asks to stay awake for one timestep after a note.
#[derive(Clone, Default)]
struct Probe {
    name: &'static str,
    pending: bool,
}

impl Work for Probe {
    fn initialize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        log(tx, self.name, "initialize")
    }
    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        log(tx, self.name, "start")
    }
    fn pre_process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        log(tx, self.name, "pre-process")
    }
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        log(tx, self.name, "process")
    }
    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if std::mem::take(&mut self.pending) {
            tx.stay_awake();
        }
        log(tx, self.name, "stop")
    }
    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        log(tx, self.name, "finalize")
    }
    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, _: Value) -> Run<()> {
        self.pending = true;
        log(tx, self.name, "hears")
    }
}

const ALL: u8 = INITIALIZE | START | PRE_PROCESS | PROCESS | STOP | FINALIZE;

const fn probe(name: &'static str, make: fn() -> Box<dyn Work>) -> TranslatorClass {
    TranslatorClass {
        name,
        kind: Kind::Engraver,
        methods: ALL,
        always: false,
        hears: &["note-event"],
        acknowledges: &[],
        must_be_last: false,
        make: Some(make),
        reads: &[],
        writes: &[],
    }
}

static PROBE_VOICE: TranslatorClass = probe("Probe_voice", || {
    Box::new(Probe {
        name: "voice",
        ..Probe::default()
    })
});
static PROBE_SCORE: TranslatorClass = TranslatorClass {
    hears: &[],
    always: true,
    ..probe("Probe_score", || {
        Box::new(Probe {
            name: "score",
            ..Probe::default()
        })
    })
};
static PROBE_LAST: TranslatorClass = TranslatorClass {
    hears: &[],
    must_be_last: true,
    methods: INITIALIZE | FINALIZE,
    ..probe("Probe_last", || {
        Box::new(Probe {
            name: "last",
            ..Probe::default()
        })
    })
};

/// Makes a `NoteHead` of each note it hears, which it acknowledges itself
/// too (it is skipped as its origin), and ends it at the next timestep.
#[derive(Clone, Default)]
struct NoteMaker {
    heard: Vec<Value>,
    made: Vec<u32>,
}

impl Work for NoteMaker {
    fn listen(&mut self, _: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        self.heard.push(ev);
        Ok(())
    }
    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for g in std::mem::take(&mut self.made) {
            tx.announce_end_grob(g);
        }
        Ok(())
    }
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for ev in std::mem::take(&mut self.heard) {
            let name = tx.heap().intern("NoteHead");
            let ifaces = eval_list(tx, &["note-head-interface", "grob-interface"]);
            let g = tx.make_grob(name, Cause::Event(ev), ifaces);
            self.made.push(g);
            tx.stay_awake();
            let audio = tx.heap().intern("Audio_note");
            let pitch = event_property(tx.heap(), ev, "pitch");
            tx.make_audio(audio, Cause::Event(ev), pitch);
        }
        Ok(())
    }
    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, _: u32) -> Run<()> {
        log(tx, "maker", "acknowledges its own")
    }
    fn values(&self, out: &mut Vec<Value>) {
        out.extend_from_slice(&self.heard);
    }
}

fn eval_list(tx: &mut Tx<'_>, names: &[&str]) -> Value {
    let items: Vec<Value> = names.iter().map(|n| tx.heap().intern(n)).collect();
    tx.heap().list(&items)
}

static NOTE_MAKER: TranslatorClass = TranslatorClass {
    name: "Note_maker",
    kind: Kind::Translator,
    methods: START | PROCESS,
    always: false,
    hears: &["note-event"],
    acknowledges: &[("note-head-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(NoteMaker::default())),
    reads: &[],
    writes: &[],
};

/// In the Staff: acknowledges the start and end of a note head, and its
/// `process_acknowledged` runs only when it acknowledged one.
#[derive(Clone)]
struct Acknowledger;

impl Work for Acknowledger {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let what = if which == 0 { "start" } else { "end" };
        let a = tx.t.arena.borrow();
        let g = &a.records.grobs[grob as usize];
        let what = match tx.heap().name(g.name) {
            Some("NoteHead") => String::from(what),
            name => format!("{what} {}", name.unwrap_or_default()),
        };
        assert!(matches!(g.cause, Cause::Event(_)));
        drop(a);
        log(tx, "acknowledger", &what)
    }
    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        log(tx, "acknowledger", "process-acknowledged")
    }
}

static ACKNOWLEDGER: TranslatorClass = TranslatorClass {
    name: "Acknowledger",
    kind: Kind::Engraver,
    methods: PROCESS_ACKNOWLEDGED,
    always: false,
    hears: &[],
    acknowledges: &[("note-head-interface", false), ("grob-interface", true)],
    must_be_last: false,
    make: Some(|| Box::new(Acknowledger)),
    reads: &[],
    writes: &[],
};

/// A performer that logs each audio item it is told of.
#[derive(Clone)]
struct Listener;

impl Work for Listener {
    fn acknowledge_audio(&mut self, tx: &mut Tx<'_>, audio: u32) -> Run<()> {
        let value = tx.t.arena.borrow().records.audio[audio as usize].value;
        let mut text = String::from("audio ");
        tx.heap().write(value, &mut text);
        log(tx, "performer", &text)
    }
}

static AUDIO_LISTENER: TranslatorClass = TranslatorClass {
    name: "Audio_listener",
    kind: Kind::Performer,
    methods: ACKNOWLEDGE_AUDIO,
    always: false,
    hears: &[],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(Listener)),
    reads: &[],
    writes: &[],
};

/// In the Staff: makes an `Accidental` of each note head it acknowledges
/// through the note head's engraver, as `Accidental_engraver` does
/// (`accidental-engraver.cc:296`).
#[derive(Clone, Default)]
struct Borrower {
    heads: Vec<u32>,
}

impl Work for Borrower {
    fn acknowledge(&mut self, _: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        self.heads.push(grob);
        Ok(())
    }
    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for head in std::mem::take(&mut self.heads) {
            let origin = tx.t.arena.borrow().records.grobs[head as usize].origin;
            assert_ne!(origin, tx.origin());
            tx.make_defined_grob_from(origin, "Accidental", Cause::Grob(head))?;
        }
        Ok(())
    }
}

static BORROWER: TranslatorClass = TranslatorClass {
    name: "Borrower",
    kind: Kind::Engraver,
    methods: PROCESS_ACKNOWLEDGED,
    always: false,
    hears: &[],
    acknowledges: &[("note-head-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(Borrower::default())),
    reads: &[],
    writes: &[],
};

static TEST_CLASSES: &[&TranslatorClass] = &[
    &PROBE_VOICE,
    &PROBE_SCORE,
    &PROBE_LAST,
    &NOTE_MAKER,
    &ACKNOWLEDGER,
    &AUDIO_LISTENER,
    &BORROWER,
];

/// `music` translated with the test classes and traced.
fn translate(vm: &mut Vm, layout: Value, music: &str) -> (Translated, String) {
    let music = eval(vm, music);
    vm.heap_mut().root(music);
    let mut trace = Trace::default();
    let done = run_translator_with(vm, music, layout, Some(&mut trace), TEST_CLASSES)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    (done, trace.text)
}

/// What the probes logged, oldest first, `name what moment` each.
fn probes(vm: &mut Vm, name: &str) -> Vec<String> {
    let list = eval(
        vm,
        &format!(
            "(map (lambda (p) (format #f \"~a ~a\" (cadr p) (ly:moment-main (caddr p))))
                  (filter (lambda (p) (eq? (car p) '{name})) (reverse probes)))"
        ),
    );
    let mut out = Vec::new();
    let mut p = list;
    while let (Some(s), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
        out.push(vm.heap().str(s).unwrap().as_str().to_string());
        p = rest;
    }
    out
}

/// What the warnings kept in `%diagnostics`, the oldest first.
fn diagnostics(vm: &mut Vm) -> String {
    let v = eval(
        vm,
        "(let ((l (reverse (car %diagnostics)))) (set-car! %diagnostics '())
           (map (lambda (d) (vector-ref d 2)) l))",
    );
    show(vm, v)
}

/// The contexts of a translation in the order they were made, removed
/// ones among them, by name, with their translators.
fn groups(vm: &Vm, done: &Translated) -> Vec<(String, Vec<String>)> {
    let a = done.translation.arena.borrow();
    (0..a.context_count() as u32)
        .map(|c| {
            let mut name = String::new();
            vm.heap().display(a.name(vm.heap(), c), &mut name);
            (name, a.translator_names(c))
        })
        .collect()
}

#[test]
fn groups_are_made_in_lilyponds_order_for_their_output() {
    let Some((mut vm, layout)) = vm() else { return };
    let (done, _) = translate(&mut vm, layout, "(note 0 2)");
    let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    // Consisted order, a must-be-last one after the others; the Staff's
    // performer filtered out of an engraver group.
    assert_eq!(
        groups(&vm, &done),
        [
            ("Global".into(), vec![]),
            (
                "Score".into(),
                names(&["Timing_translator", "Probe_score", "Probe_last"])
            ),
            (
                "Staff".into(),
                names(&["Acknowledger", "Clef_engraver", "Axis_group_engraver"])
            ),
            ("Voice".into(), names(&["Note_maker", "Probe_voice"])),
        ]
    );
    assert_eq!(diagnostics(&mut vm), "()");
    // An unknown name is warned of twice, as LilyPond does; a name
    // `ly:register-translator` keeps makes a Scheme translator of what its
    // creator gives; `\with` adds to a context's own.
    eval(
        &mut vm,
        "(ly:register-translator (lambda (c) '()) 'Scheme_probe_engraver)",
    );
    define(
        &mut vm,
        layout,
        "((context-name Voice) (translator-type Engraver_group) (consists Nowhere_engraver)
          (consists Scheme_probe_engraver))",
    );
    let (done, _) = translate(
        &mut vm,
        layout,
        "(make-music 'ContextSpeccedMusic 'context-type 'Voice 'create-new #t
           'property-operations '((consists Tie_engraver)) 'element (note 0 2))",
    );
    let voice = groups(&vm, &done).pop().unwrap();
    assert_eq!(voice.0, "Voice");
    assert_eq!(voice.1.len(), 2);
    assert_eq!(voice.1[0], "Scheme_engraver");
    assert_eq!(voice.1[1], "Tie_engraver");
    assert_eq!(
        diagnostics(&mut vm),
        "(\"unknown translator: `Nowhere_engraver'\" \"cannot find: `Nowhere_engraver'\")"
    );
}

#[test]
fn the_protocol_runs_translators_with_work() {
    let Some((mut vm, layout)) = vm() else { return };
    // A note at 0 and 1/2, a rest at 1/4: the voice's probe is woken by its
    // notes, stays awake for the timestep after each, and sleeps at 3/4.
    translate(
        &mut vm,
        layout,
        "(seq (note 0 2) (rest 2) (note 0 2) (rest 2) (rest 2))",
    );
    assert_eq!(
        probes(&mut vm, "voice"),
        [
            "initialize 0",
            "hears 0",
            "pre-process 0",
            "process 0",
            "stop 0",
            "start 1/4",
            "pre-process 1/4",
            "process 1/4",
            "stop 1/4",
            "hears 1/2",
            "pre-process 1/2",
            "process 1/2",
            "stop 1/2",
            "start 3/4",
            "pre-process 3/4",
            "process 3/4",
            "stop 3/4",
            "finalize 5/4",
        ]
    );
    // The Score's runs always, its first timestep but the start: it was
    // made after the Prepare. The bar start at 1/2 is Timing_translator's.
    let score = probes(&mut vm, "score");
    assert_eq!(score[..3], ["initialize 0", "pre-process 0", "process 0"]);
    for m in ["1/4", "1/2", "3/4", "1", "5/4"] {
        assert!(score.contains(&format!("start {m}")), "{m}: {score:?}");
    }
    assert_eq!(score.last().unwrap(), "finalize 5/4");
    // Two methods only.
    assert_eq!(probes(&mut vm, "last"), ["initialize 0", "finalize 5/4"]);
}

#[test]
fn acknowledgers_take_the_grobs_announced_below_them() {
    let Some((mut vm, layout)) = vm() else { return };
    let (done, _) = translate(&mut vm, layout, "(seq (note 0 2) (note 1 2))");
    // The Staff hears each note head start at its note, and end at the
    // next timestep; its process_acknowledged runs after each, and not
    // where nothing was announced. The maker is not told of its own.
    assert_eq!(
        probes(&mut vm, "acknowledger"),
        [
            "start 0",
            "process-acknowledged 0",
            "end 1/4",
            "start 1/4",
            "process-acknowledged 1/4",
            "end 1/2",
            "process-acknowledged 1/2",
        ]
    );
    assert!(probes(&mut vm, "maker").is_empty());
    let a = done.translation.arena.borrow();
    let grobs = &a.records.grobs;
    assert_eq!(grobs.len(), 2);
    let ended: Vec<_> = grobs
        .iter()
        .map(|g| {
            (
                g.moment.main.numerator(),
                g.ended.map(|m| m.main.denominator()),
            )
        })
        .collect();
    assert_eq!(ended, [(0, Some(4)), (1, Some(2))]);
    assert_eq!(a.records.audio.len(), 2);
}

#[test]
fn performer_groups_tell_their_performers_of_audio() {
    let Some((mut vm, layout)) = vm() else { return };
    // A MIDI Score of Timing_translator and a Staff of the listener, whose
    // engraver is filtered out; the maker in a Voice of a plain group
    // announces to the performer groups above it.
    define(
        &mut vm,
        layout,
        "((context-name Score) (translator-type Score_performer) (accepts Staff)
          (default-child Staff) (assign timing #t) (assign timeSignature (2 . 4))
          (assign timeSignatureSettings ((2 . 4))) (consists Timing_translator))",
    );
    define(
        &mut vm,
        layout,
        "((context-name Staff) (translator-type Performer_group) (accepts Voice)
          (default-child Voice) (consists Audio_listener) (consists Clef_engraver))",
    );
    define(
        &mut vm,
        layout,
        "((context-name Voice) (translator-type Performer_group) (consists Note_maker))",
    );
    let (done, _) = translate(&mut vm, layout, "(seq (note 0 2) (note 2 2))");
    assert_eq!(
        groups(&vm, &done)[2],
        ("Staff".into(), vec!["Audio_listener".into()])
    );
    assert_eq!(
        probes(&mut vm, "performer"),
        ["audio #<Pitch c' > 0", "audio #<Pitch e' > 1/4"]
    );
}

#[test]
fn timing_translator_keeps_bars_and_asks_for_bar_starts() {
    let Some((mut vm, layout)) = vm() else { return };
    // 2/4: a whole note from 0 crosses the bar at 1/2, which is a timestep
    // without music; the trace's properties are written at each bar start.
    let (done, trace) = translate(&mut vm, layout, "(seq (note 0 0) (note 0 1))");
    let bars: Vec<&str> = trace
        .lines()
        .filter(|l| l.starts_with("S\t"))
        .map(|l| l.split('\t').nth(1).unwrap())
        .collect();
    assert_eq!(bars, ["0", "1/2", "1", "3/2"]);
    let score = done.translation.arena.borrow().children(GLOBAL)[0];
    let get = |vm: &mut Vm, name: &str| {
        let sym = vm.heap_mut().intern(name);
        let v = done.translation.arena.borrow().here(score, sym);
        v.map(|v| show(vm, v)).unwrap_or_default()
    };
    // Its initialization from the stand-ins, and where the last timestep
    // left it: the bar at 3/2 is the fourth.
    assert_eq!(get(&mut vm, "measureLength"), "1/2");
    assert_eq!(get(&mut vm, "beamExceptions"), "(exceptions (2 . 4))");
    assert_eq!(get(&mut vm, "beatBase"), "1/4");
    assert_eq!(get(&mut vm, "beatStructure"), "(beats 1/4)");
    assert_eq!(get(&mut vm, "submeasureStructure"), "(2)");
    assert_eq!(get(&mut vm, "currentBarNumber"), "4");
    assert_eq!(get(&mut vm, "internalBarNumber"), "4");
    assert_eq!(get(&mut vm, "measurePosition"), "#<Mom 0>");
    assert_eq!(get(&mut vm, "measureStartNow"), "#t");
    assert_eq!(get(&mut vm, "whichBar"), "()");
    let heap = vm.heap_mut();
    let timing = heap.intern("Timing");
    assert!(
        done.translation
            .arena
            .borrow()
            .is_alias(heap, score, timing)
    );
    assert_eq!(diagnostics(&mut vm), "()");
}

#[test]
fn timing_translator_hears_partial_bars_and_bar_checks() {
    let Some((mut vm, layout)) = vm() else { return };
    // \partial 4 at the start: the first bar is a quarter long, and bars
    // start at 1/4, 3/4 and 5/4; a bar check at 7/8, inside a bar, warns,
    // and one at 5/4, a bar start, does not.
    let (_, trace) = translate(
        &mut vm,
        layout,
        "(seq (event 'PartialSet 'duration (ly:make-duration 2))
              (note 0 2) (note 0 1) (event 'BarEvent 'bar-type \"||\")
              (note 0 3) (event 'BarCheckEvent) (note 0 3) (note 0 2) (event 'BarCheckEvent) (note 0 2))",
    );
    let bars: Vec<&str> = trace
        .lines()
        .filter(|l| l.starts_with("S\t"))
        .map(|l| l.split('\t').nth(1).unwrap())
        .collect();
    assert_eq!(bars, ["1/4", "3/4", "5/4"]);
    assert_eq!(diagnostics(&mut vm), "(\"bar check failed at: 1/8\")");
}

#[test]
fn the_scheme_face_names_classes_and_registered_translators() {
    let Some((mut vm, layout)) = vm() else { return };
    let all = eval(&mut vm, "(length (ly:get-all-translators))");
    assert_eq!(show(&vm, all), "128");
    let timing = eval(
        &mut vm,
        "(let ((c (car (filter (lambda (c) (eq? (ly:translator-name c) 'Timing_translator))
                               (ly:get-all-translators)))))
           (list (ly:translator-name c) (assq-ref (ly:translator-description c) 'events-accepted)))",
    );
    assert_eq!(
        show(&vm, timing),
        "(Timing_translator (alternative-event bar-event bar-check-event fine-event \
         partial-event polymetric-time-signature-event))"
    );
    eval(
        &mut vm,
        "(define-public creator (lambda (c) '()))
         (ly:register-translator creator 'Scheme_probe '((description . \"d\")))",
    );
    let named = eval(
        &mut vm,
        "(list (ly:translator-name creator) (ly:translator-description creator)
               (and (memq creator (ly:get-all-translators)) #t))",
    );
    assert_eq!(
        show(&vm, named),
        "(Scheme_probe ((description . \"d\")) #t)"
    );
    let error = vm.eval_text("(ly:translator-name 'x)", None).unwrap_err();
    assert_eq!(
        error.text(vm.heap()),
        "In procedure ly_translator_name: Wrong type argument in position 1 \
         (expecting translator definition): x"
    );
    // A translator's handle and its context.
    let (done, _) = translate(&mut vm, layout, "(note 0 2)");
    let t = &done.translation;
    let score = t.arena.borrow().children(GLOBAL)[0];
    let handle = translator_handle(vm.heap_mut(), t, score, 0).unwrap();
    assert_eq!(
        handle,
        translator_handle(vm.heap_mut(), t, score, 0).unwrap()
    );
    vm.define("test-translator", handle);
    let v = eval(
        &mut vm,
        "(list (ly:translator? test-translator) (ly:translator? 1)
               (ly:context-name (ly:translator-context test-translator))
               (object->string test-translator))",
    );
    assert_eq!(
        show(&vm, v),
        "(#t #f Score \"#<Translator Timing_translator >\")"
    );
}

/// A Voice of the test classes and the Scheme translators `names`, which
/// `ly:register-translator` keeps.
fn scheme_voice(vm: &mut Vm, layout: Value, names: &str) {
    define(
        vm,
        layout,
        &format!(
            "((context-name Voice) (translator-type Engraver_group) (consists Note_maker) {names})"
        ),
    );
}

#[test]
fn scheme_translators_run_with_the_classes_wake_rules() {
    let Some((mut vm, layout)) = vm() else { return };
    // `make-engraver`'s alists by hand: one of every method and a
    // listener, run always; one of a listener only, run when it hears; a
    // performer, filtered out of an engraver group; and a must-be-last one.
    eval(
        &mut vm,
        "(define (at t) (ly:context-current-moment (ly:translator-context t)))
         (define (method name what)
           (cons name (lambda (t) (probe! 'every what (at t)))))
         (ly:register-translator
           (lambda (context)
             (list (method 'initialize \"initialize\") (method 'start-translation-timestep \"start\")
                   (method 'pre-process-music \"pre-process\") (method 'process-music \"process\")
                   (method 'stop-translation-timestep \"stop\") (method 'finalize \"finalize\")
                   (cons 'listeners
                         (list (cons 'note-event
                                     (lambda (t ev)
                                       (probe! 'every (symbol->string (ly:context-name (ly:translator-context t)))
                                               (at t))))))
                   '(is-layout . #t)))
           'Every_engraver)
         (ly:register-translator
           (lambda (context)
             `((listeners (rest-event . ,(lambda (t ev) (probe! 'rests \"hears\" (at t)))))))
           'Rests_probe)
         (ly:register-translator
           (lambda (context) `((is-midi . #t) (initialize . ,(lambda (t) (probe! 'midi \"initialize\" (at t))))))
           'Midi_performer)
         (ly:register-translator (lambda (context) '((must-be-last . #t))) 'Last_engraver)",
    );
    scheme_voice(
        &mut vm,
        layout,
        "(consists Last_engraver) (consists Every_engraver) (consists Midi_performer) (consists Rests_probe)",
    );
    let (done, _) = translate(&mut vm, layout, "(seq (note 0 2) (rest 2) (note 0 2))");
    // In LilyPond's order (Note_maker, Every, Rests), the must-be-last one
    // last, the performer filtered out.
    assert_eq!(
        groups(&vm, &done)[3].1,
        [
            "Note_maker",
            "Scheme_engraver",
            "Scheme_engraver",
            "Scheme_engraver"
        ]
    );
    // Every timestep's methods run, the music's end at 3/4 among them, but
    // the start at 0, which comes before the Voice is made; its listener
    // runs at the notes.
    assert_eq!(
        probes(&mut vm, "every"),
        [
            "initialize 0",
            "Voice 0",
            "pre-process 0",
            "process 0",
            "stop 0",
            "start 1/4",
            "pre-process 1/4",
            "process 1/4",
            "stop 1/4",
            "start 1/2",
            "Voice 1/2",
            "pre-process 1/2",
            "process 1/2",
            "stop 1/2",
            "start 3/4",
            "pre-process 3/4",
            "process 3/4",
            "stop 3/4",
            "finalize 3/4",
        ]
    );
    assert_eq!(probes(&mut vm, "rests"), ["hears 1/4"]);
    assert!(probes(&mut vm, "midi").is_empty());
    // A translator's handle prints as LilyPond's Scheme_engraver.
    let v = eval(
        &mut vm,
        "(let ((t #f))
           (ly:register-translator (lambda (c) `((initialize . ,(lambda (x) (set! t x))))) 'Keep_engraver)
           t)",
    );
    assert_eq!(show(&vm, v), "#f");
    scheme_voice(&mut vm, layout, "(consists Keep_engraver)");
    eval(
        &mut vm,
        "(define kept #f)
         (ly:register-translator (lambda (c) `((initialize . ,(lambda (x) (set! kept x))))) 'Keep_engraver)",
    );
    translate(&mut vm, layout, "(note 0 2)");
    let v = eval(
        &mut vm,
        "(list (ly:translator? kept) (object->string kept))",
    );
    assert_eq!(show(&vm, v), "(#t \"#<Translator Scheme_engraver >\")");
}

#[test]
fn a_scheme_engraver_makes_grobs_and_acknowledges_them() {
    let Some((mut vm, layout)) = vm() else { return };
    define(
        &mut vm,
        layout,
        "((context-name Global) (accepts Score) (default-child Score)
          (grob-descriptions ((TextScript (direction . 1)
                                          (meta (name . TextScript) (interfaces text-script-interface grob-interface))))))",
    );
    // It makes a TextScript of each note, which the Staff's acknowledger
    // hears end where it ends it, and acknowledges the note heads
    // Note_maker makes, start and end.
    eval(
        &mut vm,
        "(define (at t) (ly:context-current-moment (ly:translator-context t)))
         (ly:register-translator
           (lambda (context)
             (let ((heard #f) (made #f))
               `((listeners (note-event . ,(lambda (t ev) (set! heard ev))))
                 (process-music
                  . ,(lambda (t)
                       (when made (ly:engraver-announce-end-grob t made '()) (set! made #f))
                       (when heard
                         (set! made (ly:engraver-make-grob t 'TextScript heard))
                         (ly:grob-set-property! made 'text \"hi\")
                         (probe! 'scheme (format #f \"made ~a ~a ~a\" made (ly:grob-property made 'text)
                                                 (ly:grob-property made 'direction))
                                 (at t))
                         (set! heard #f))))
                 (acknowledgers
                  (note-head-interface
                   . ,(lambda (t grob source)
                        (probe! 'scheme (format #f \"saw ~a from ~a\" grob source) (at t)))))
                 (end-acknowledgers
                  (note-head-interface
                   . ,(lambda (t grob source) (probe! 'scheme \"saw end\" (at t))))))))
           'Text_maker)",
    );
    scheme_voice(&mut vm, layout, "(consists Text_maker)");
    let (done, _) = translate(&mut vm, layout, "(seq (note 0 2) (note 1 2))");
    assert_eq!(
        probes(&mut vm, "scheme"),
        [
            "made #<Grob TextScript > hi 1 0",
            "saw #<Grob NoteHead > from #<Translator Note_maker > 0",
            "made #<Grob TextScript > hi 1 1/4",
            "saw end 1/4",
            "saw #<Grob NoteHead > from #<Translator Note_maker > 1/4",
            "saw end 1/2",
        ]
    );
    // The Staff's acknowledger takes the note heads' starts and every
    // grob's end, the TextScripts' among them, in the order they were
    // announced: Note_maker runs before the Scheme engraver.
    assert_eq!(
        probes(&mut vm, "acknowledger"),
        [
            "start 0",
            "process-acknowledged 0",
            "end 1/4",
            "start 1/4",
            "end TextScript 1/4",
            "process-acknowledged 1/4",
            "end 1/2",
            "end TextScript 1/2",
            "process-acknowledged 1/2",
        ]
    );
    let a = done.translation.arena.borrow();
    let names: Vec<_> = a.records.grobs.iter().map(|g| show(&vm, g.name)).collect();
    assert_eq!(names, ["NoteHead", "TextScript", "NoteHead", "TextScript"]);
    assert!(matches!(a.records.grobs[1].cause, Cause::Event(_)));
    let error = vm
        .eval_text("(ly:engraver-make-grob 1 'TextScript '())", None)
        .unwrap_err();
    assert_eq!(
        error.text(vm.heap()),
        "In procedure ly:engraver-make-grob: Wrong type argument in position 1 (expecting Engraver): 1"
    );
}

#[test]
fn a_record_is_made_through_another_translator_as_its_own() {
    let Some((mut vm, layout)) = vm() else { return };
    define(
        &mut vm,
        layout,
        "((context-name Global) (accepts Score) (default-child Score)
          (grob-descriptions ((Accidental (color . red)
                                          (meta (name . Accidental) (interfaces accidental-interface grob-interface))))))",
    );
    define(
        &mut vm,
        layout,
        "((context-name Staff) (translator-type Engraver_group) (accepts Voice)
          (default-child Voice) (consists Borrower) (push Accidental green color))",
    );
    // A Scheme engraver in the Voice hears each Accidental announced there,
    // from Note_maker.
    eval(
        &mut vm,
        "(set-object-property! 'color 'backend-type? symbol?)
         (ly:register-translator
           (lambda (context)
             `((acknowledgers
                (accidental-interface
                 . ,(lambda (t grob source)
                      (probe! 'scheme (format #f \"saw ~a from ~a ~a\" grob source
                                              (ly:grob-property grob 'color))
                              (ly:context-current-moment (ly:translator-context t))))))))
           'Accidental_hearer)",
    );
    scheme_voice(
        &mut vm,
        layout,
        "(consists Accidental_hearer) (push Accidental blue color)",
    );
    let (done, _) = translate(&mut vm, layout, "(seq (note 0 2) (note 1 2))");
    assert_eq!(
        probes(&mut vm, "scheme"),
        [
            "saw #<Grob Accidental > from #<Translator Note_maker > blue 0",
            "saw #<Grob Accidental > from #<Translator Note_maker > blue 1/4",
        ]
    );
    // Each Accidental is Note_maker's in the Voice, its definition seen
    // from the Voice (blue, not the Staff's green), its cause the head.
    let a = done.translation.arena.borrow();
    let grobs = &a.records.grobs;
    let names: Vec<_> = grobs.iter().map(|g| show(&vm, g.name)).collect();
    assert_eq!(names, ["NoteHead", "Accidental", "NoteHead", "Accidental"]);
    for (head, acc) in [(0, 1), (2, 3)] {
        let (h, g) = (&grobs[head], &grobs[acc]);
        assert_eq!(g.origin, h.origin);
        assert_eq!(a.translator_name(g.origin), "Note_maker");
        assert_eq!(show(&vm, a.name(vm.heap(), g.origin.0)), "Voice");
        assert_eq!(g.cause, Cause::Grob(head as u32));
        assert_eq!(g.moment, h.moment);
    }
    let color = vm.heap_mut().intern("color");
    let blue = vm.heap_mut().intern("blue");
    let first = vm.heap().car(grobs[1].definition).unwrap();
    let first = (vm.heap().car(first), vm.heap().cdr(first));
    assert_eq!(first, (Some(color), Some(blue)));
    assert_eq!(diagnostics(&mut vm), "()");
}

#[test]
fn a_performance_writes_the_playback_map() {
    let Some((mut vm, layout)) = vm() else { return };
    // A MIDI score of two staves in a piano staff: the first named, the
    // second of no notes, so of no clef or voices; events at a location.
    for text in [
        "((context-name Score) (translator-type Score_performer) (accepts PianoStaff)
          (default-child PianoStaff) (assign timing #t) (assign timeSignature (2 . 4))
          (assign timeSignatureSettings ((2 . 4))) (consists Timing_translator))",
        "((context-name PianoStaff) (translator-type Performer_group) (accepts Staff)
          (default-child Staff))",
        "((context-name Staff) (translator-type Performer_group) (accepts Voice)
          (default-child Voice) (assign instrumentName \"bright acoustic\"))",
        "((context-name Voice) (translator-type Performer_group))",
    ] {
        define(&mut vm, layout, text);
    }
    let at = vm
        .heap_mut()
        .wrap(Input::new(0, 0, 4, "/s/a b.ly".into(), [3, 4, 6]));
    vm.define("loc", at);
    eval(
        &mut vm,
        "(set-object-property! 'instrumentName 'translation-type? string?)
         (set-object-property! 'clefGlyph 'translation-type? string?)",
    );
    let music = eval(
        &mut vm,
        "(define (n step log) (make-music 'NoteEvent 'pitch (ly:make-pitch 0 step 0)
                                          'duration (ly:make-duration log) 'origin loc))
         (define (set-in type prop value)
           (make-music 'ContextSpeccedMusic 'context-type type
             'element (make-music 'PropertySet 'symbol prop 'value value)))
         (new 'PianoStaff
           (sim (new 'Staff (seq (set-in 'Staff 'instrumentName \"Flute\") (n 0 2) (n 1 1)))
                (new 'Staff (seq (set-in 'Staff 'clefGlyph \"clefs.F\")
                                 (make-music 'RestEvent 'duration (ly:make-duration 2) 'origin loc)
                                 (make-music 'LyricEvent 'text \"la\" 'duration (ly:make-duration 2) 'origin loc)))))",
    );
    vm.heap_mut().root(music);
    let done = run_performance(&mut vm, music, layout)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    let event = |at: &str, length: &str| {
        format!(
            r#"{{"href":"textedit:///s/a%20b.ly:3:4:7","at":{at},"grace":0,"length":{length}}}"#
        )
    };
    let staff = |name: &str, clef: &str| {
        format!(
            r#"{{"id":"","name":"{name}","shortName":"","group":"PianoStaff","groupIndex":0,"clef":"{clef}","clefTransposition":0,"voices":[""]}}"#
        )
    };
    // A syllable's length is 0; bars where currentBarNumber changes, the
    // second at 1/2.
    assert_eq!(
        done.playback().unwrap(),
        format!(
            r#"{{"events":[{},{},{},{}],"bars":[{{"at":0,"number":1}},{{"at":0.5,"number":2}}],"staves":[{},{}]}}"#,
            event("0", "0.25"),
            event("0", "0.25"),
            event("0.25", "0.5"),
            event("0.25", "0"),
            staff("Flute", "clefs.G"),
            staff("", "").replace(r#""voices":[""]"#, r#""voices":[]"#),
        )
    );
    // A \layout translation has no playback map.
    let (done, _) = translate(&mut vm, layout, "(note 0 2)");
    assert_eq!(done.playback(), None);
}
