//! H3 (D254): a compile returns between its steps, and a newer one stops it.
//! A file of its own, whose tests run one at a time ([`alone`]): a step's
//! deadline is the process's ([`lily_engine_music::returns`]), and a test on
//! another thread would move it.

use lily_engine_core::{
    DONE, Engine, FILE, MODULE, MORE, NEEDS, PART_MODULES, write_image, write_part,
};
use lily_engine_parser::source::DATADIR;
use lily_engine_scheme::Vm;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

#[allow(dead_code)]
#[path = "../../parser/tests/support/mod.rs"]
mod support;

/// This test alone in the process, until it ends.
fn alone() -> MutexGuard<'static, ()> {
    static ONE: Mutex<()> = Mutex::new(());
    ONE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn engine_of(mut vm: Vm) -> Engine {
    let image = vm.write_image().expect("an image of the VM");
    Engine::start(&write_image(&image)).expect("the image restores")
}

/// A compile of `root` from disk, with each step's `budget`; how many times
/// it returned.
fn answered(engine: &mut Engine, root: &str, datadir: &Path, budget: f64) -> usize {
    answered_by_stage(engine, root, datadir, budget).len()
}

/// [`answered`], with what the compile went on with at each return.
fn answered_by_stage(
    engine: &mut Engine,
    root: &str,
    datadir: &Path,
    budget: f64,
) -> Vec<&'static str> {
    answered_with(engine, root, datadir, budget, |_| {})
}

/// [`answered_by_stage`], `at_return` given the engine at each return.
fn answered_with(
    engine: &mut Engine,
    root: &str,
    datadir: &Path,
    budget: f64,
    mut at_return: impl FnMut(&mut Engine),
) -> Vec<&'static str> {
    let fonts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts");
    let mut returns = Vec::new();
    engine.compile(1, root);
    loop {
        match engine.step(budget) {
            MORE => {
                returns.push(engine.stage());
                at_return(engine);
            }
            NEEDS => {
                let (kind, name) = engine.needs().map(|(k, n)| (k, n.to_string())).unwrap();
                let data = if kind == MODULE && PART_MODULES.iter().any(|(p, _)| *p == name) {
                    Some(part_module(&name, datadir))
                } else if kind == MODULE {
                    std::fs::read(fonts.join(format!("{name}.pack"))).ok()
                } else {
                    let rest = name.strip_prefix(DATADIR).unwrap_or(&name);
                    std::fs::read(datadir.join(rest.trim_start_matches('/'))).ok()
                };
                engine.put(kind, &name, data.as_deref());
                engine.compile(1, root);
            }
            _ => return returns,
        }
    }
}

/// The module of one of [`PART_MODULES`], its files from `datadir`'s
/// `scm/lily/`, as the build writes it (`engine.rs`').
fn part_module(part: &str, datadir: &Path) -> Vec<u8> {
    let (_, files) = PART_MODULES.iter().find(|(p, _)| *p == part).unwrap();
    let texts: Vec<Vec<u8>> = files
        .iter()
        .map(|file| std::fs::read(datadir.join("scm/lily").join(file)).unwrap())
        .collect();
    let named: Vec<(&str, &[u8])> = files
        .iter()
        .copied()
        .zip(texts.iter().map(Vec::as_slice))
        .collect();
    write_part(&named)
}

/// A compile returns between its timesteps, grobs, lines, systems and pages,
/// and writes what the one call writes; a newer compile stops it wherever it
/// is, and nothing of it shows in that one.
#[test]
fn a_compile_returns_between_its_steps_and_a_newer_one_stops_it() {
    let _alone = alone();
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let datadir = lily.join("../..");
    let score = |notes: &str| {
        format!(
            "\\version \"2.26.0\"
\\paper {{ page-count = 3 }}
\\score {{ \\new Staff {{ \\repeat unfold 80 {{ {notes} | }} }} \\layout {{ }} \\midi {{ }} }}
"
        )
    };
    let (text, other) = (score("c'8 d' e' f' g' a' b' c''"), score("g'4 a' b' c''"));
    let fresh = |returns: bool| {
        let mut engine =
            engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
        engine.set_returns(returns);
        engine
    };
    let whole = |text: &str| {
        let mut engine = fresh(false);
        engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
        answered(&mut engine, "/s/a.ly", &datadir, 0.0);
        engine.outputs().to_vec()
    };
    let one_call = whole(&text);
    let pages = one_call.iter().filter(|(n, _)| n.ends_with(".svg")).count();
    assert_eq!(pages, 3);

    let mut stepped = fresh(true);
    stepped.put(FILE, "/s/a.ly", Some(text.as_bytes()));
    let returns = answered(&mut stepped, "/s/a.ly", &datadir, 0.0);
    assert!(returns > 500, "it returned {returns} times");
    assert!(!stepped.running(), "a compile done is not under way");
    assert_eq!(stepped.outputs(), one_call);
    assert_eq!(stepped.step(0.0), DONE, "a compile done stays done");

    // The same text again returns less often: it is what a newer one stops.
    let again = answered(&mut stepped, "/s/a.ly", &datadir, 0.0);
    assert!(again > 3, "it returned {again} times");

    // Stopped early, in the middle and late, by another text.
    for at in [1, again / 2, again - 1] {
        let mut engine = fresh(true);
        engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
        answered(&mut engine, "/s/a.ly", &datadir, 0.0);
        engine.compile(2, "/s/a.ly");
        for _ in 0..at {
            assert_eq!(engine.step(0.0), MORE);
        }
        assert!(engine.running(), "stopped at {at} of {again}");
        engine.put(FILE, "/s/a.ly", Some(other.as_bytes()));
        assert!(!engine.running(), "a newer file stops it");
        answered(&mut engine, "/s/a.ly", &datadir, 0.0);
        let stopped = engine.outputs();
        assert_eq!(stopped, whole(&other), "stopped at {at} of {again}");
    }

    // Stopped by the same text, by a compile alone.
    let mut engine = fresh(true);
    engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
    answered(&mut engine, "/s/a.ly", &datadir, 0.0);
    engine.compile(2, "/s/a.ly");
    assert_eq!(engine.step(0.0), MORE);
    answered(&mut engine, "/s/a.ly", &datadir, 0.0);
    assert_eq!(engine.outputs(), one_call);
}

/// A score of `n` variables, each a staff's music, the `k`th of them
/// `notes`, and one score of them all: a parse of `n + 3` segments.
fn variables(n: usize, k: usize, notes: &str) -> String {
    // What a stopped parse left in `(lily)` would be warned of.
    let mut text = String::from(
        "\\version \"2.26.0\"\n#(if (module-ref (resolve-module '(lily)) 'leaked #f) (ly:warning \"leaked\"))\n",
    );
    for i in 0..n {
        let music = if i == k { notes } else { "c'4 d' e' f'" };
        let name: String = (0..3)
            .map(|d| (b'a' + (i / 26usize.pow(d) % 26) as u8) as char)
            .collect();
        text += &format!("v{name} = \\relative {{ \\repeat unfold 4 {{ {music} | }} }}\n");
    }
    text += "\\score { <<\n";
    for i in 0..n {
        let name: String = (0..3)
            .map(|d| (b'a' + (i / 26usize.pow(d) % 26) as u8) as char)
            .collect();
        text += &format!("  \\new Staff \\v{name}\n");
    }
    text + ">> \\layout { } }\n"
}

/// The parse returns between its segments (D267), and what it writes is the
/// one call's; a newer edit stops it at any of them, and the next parse,
/// which may do again what the stopped one parsed, is the whole one's.
#[test]
fn a_parse_returns_between_its_segments_and_a_newer_edit_stops_it() {
    let _alone = alone();
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let datadir = lily.join("../..");
    let fresh = |returns: bool| {
        let mut engine =
            engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
        engine.set_returns(returns);
        engine
    };
    let whole = |text: &str| {
        let mut engine = fresh(false);
        engine.put(FILE, "/s/v.ly", Some(text.as_bytes()));
        answered(&mut engine, "/s/v.ly", &datadir, 0.0);
        engine.outputs().to_vec()
    };
    let n = 12;
    let text = variables(n, 2, "e'4 f' g' a'");
    // A variable a segment defines in `(lily)`, which the stopped parse's
    // undo takes back.
    let early = variables(n, 2, "g'4 a' b' c''").replacen(
        "\n",
        "\n#(module-define! (resolve-module '(lily)) 'leaked #t)\n",
        1,
    );
    let late = variables(n, 10, "g'4 a' b' c''");
    let one_call = whole(&text);

    // Each step of a budget of 0 parses one segment: the parse is `n` steps
    // or more before the compile's.
    let mut stepped = fresh(true);
    stepped.put(FILE, "/s/v.ly", Some(text.as_bytes()));
    let stages = answered_by_stage(&mut stepped, "/s/v.ly", &datadir, 0.0);
    let parsing = stages.iter().filter(|&&s| s == "parsing").count();
    assert!(parsing >= n, "the parse returned {parsing} times");
    assert_eq!(stepped.outputs(), one_call);

    // Another text's parse stopped early, in the middle and late, by an edit
    // before where it stopped, one after, and the text before it.
    let first = variables(n, 0, "a'4 b' c'' d''");
    let leaked = |outputs: &[(String, Vec<u8>)]| {
        outputs
            .iter()
            .any(|(n, o)| n == "stderr" && o.windows(6).any(|w| w == b"leaked"))
    };
    assert!(!leaked(&one_call) && leaked(&whole(&early)));
    for at in [2, n / 2, n - 1] {
        for other in [&first, &late, &text] {
            let mut engine = fresh(true);
            engine.put(FILE, "/s/v.ly", Some(text.as_bytes()));
            answered(&mut engine, "/s/v.ly", &datadir, 0.0);
            engine.put(FILE, "/s/v.ly", Some(early.as_bytes()));
            engine.compile(2, "/s/v.ly");
            for _ in 0..at {
                assert_eq!(engine.step(0.0), MORE);
            }
            assert_eq!(engine.stage(), "parsing", "stopped at {at}");
            engine.put(FILE, "/s/v.ly", Some(other.as_bytes()));
            assert!(!engine.running(), "a newer file stops it");
            engine.compile(3, "/s/v.ly");
            answered(&mut engine, "/s/v.ly", &datadir, 0.0);
            assert_eq!(engine.outputs(), whole(other), "stopped at {at}");
        }
    }
}

/// A score of two staves on a page, 12 bars of `notes` in the top, four that
/// an edit can change between the snapshots of a run from the start, so that
/// it resumes before them and can splice after them, then 24 of `notes`.
fn score(notes: &str) -> String {
    format!(
        "\\version \"2.26.0\"
\\paper {{ page-count = 1 }}
\\score {{
  <<
    \\new Staff {{ \\repeat unfold 12 {{ {notes} | }} c'1 d'1 e'1 f'1 \\repeat unfold 24 {{ {notes} | }} }}
    \\new Staff {{ \\clef bass \\repeat unfold 12 {{ c2 g2 | }} c1 g1 c1 g1 \\repeat unfold 24 {{ c2 g2 | }} }}
  >>
  \\layout {{ }} \\midi {{ }}
}}
"
    )
}

/// A compile a newer one stops leaves what the compile before it kept (B24,
/// D261): wherever it stops, and however the text moved, the next compile
/// resumes it, and writes what a fresh engine writes. Two in a row too, as
/// typing is. Every stop lands in a different part of the compile: its
/// translation, a spacing read, a line's heights, the page's spacing problem,
/// a collection.
#[test]
fn a_stop_leaves_what_the_last_compile_kept_and_the_next_resumes_it() {
    let _alone = alone();
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let datadir = lily.join("../..");
    let fresh = || {
        let mut engine =
            engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
        engine.set_returns(true);
        engine
    };
    let whole = |text: &str| {
        let mut engine = fresh();
        engine.set_returns(false);
        engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
        answered(&mut engine, "/s/a.ly", &datadir, 0.0);
        engine.outputs().to_vec()
    };
    let text = score("c'4 d' e' f'");
    // Typing: each of the same bar, each moving the bytes after it, the last
    // by a line too.
    let stopped = text.replacen("d'1 e'1", "d'2 d' e'1", 1);
    let last = text.replacen("d'1 e'1", "d'2 d'4\n d' e'1", 1);
    assert!(text != stopped && stopped != last, "the edits edit");
    let expected = whole(&last);
    let same_again = whole(&stopped);
    assert!(expected.iter().any(|(n, _)| n.ends_with(".svg")));

    // One engine throughout, as a person's: each round goes back to `text`.
    let mut engine = fresh();
    let compiled = |engine: &mut Engine, text: &str| {
        engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
        answered(engine, "/s/a.ly", &datadir, 0.0)
    };
    compiled(&mut engine, &text);
    let total = compiled(&mut engine, &stopped);
    assert!(total > 40, "it returned {total} times");
    let mut resumed = 0;
    for at in (1..total).step_by(total / 12) {
        compiled(&mut engine, &text);
        engine.put(FILE, "/s/a.ly", Some(stopped.as_bytes()));
        engine.compile(2, "/s/a.ly");
        for _ in 0..at {
            if engine.step(0.0) != MORE {
                break;
            }
        }
        compiled(&mut engine, &last);
        assert_eq!(engine.outputs(), expected, "stopped after {at} of {total}");
        resumed += usize::from(engine.resumed_at().iter().any(Option::is_some));
    }
    assert!(
        resumed > 8,
        "{resumed} of its compiles after a stop resumed"
    );

    // Stopped late, the translation already spliced to the last run's end, and
    // then the same text: its parse is of that text already.
    for at in [total / 2, total - total / 8] {
        compiled(&mut engine, &text);
        engine.put(FILE, "/s/a.ly", Some(stopped.as_bytes()));
        engine.compile(2, "/s/a.ly");
        for _ in 0..at {
            assert_eq!(engine.step(0.0), MORE);
        }
        compiled(&mut engine, &stopped);
        assert_eq!(
            engine.outputs(),
            same_again,
            "stopped after {at}, the same text"
        );
        assert!(engine.resumed_at().iter().any(Option::is_some));
    }

    // A collection at each step of the compile stopped: what the stop puts
    // back must live through them, and through one before the next compile.
    for at in [3, 12, 40] {
        compiled(&mut engine, &text);
        engine.set_collect_every_step(true);
        engine.put(FILE, "/s/a.ly", Some(stopped.as_bytes()));
        engine.compile(2, "/s/a.ly");
        for _ in 0..at {
            if engine.step(20.0) != MORE {
                break;
            }
        }
        engine.put(FILE, "/s/a.ly", Some(last.as_bytes()));
        answered(&mut engine, "/s/a.ly", &datadir, 20.0);
        engine.set_collect_every_step(false);
        assert_eq!(engine.outputs(), expected, "collected, stopped after {at}");
        assert!(engine.resumed_at().iter().any(Option::is_some));
    }

    // Stopped in the page's spacing problem, a call of Scheme under way: its
    // frames go with it.
    compiled(&mut engine, &text);
    engine.put(FILE, "/s/a.ly", Some(stopped.as_bytes()));
    engine.compile(2, "/s/a.ly");
    // (The score's `scorify-music` is a call in steps too, D272.)
    while !(engine.calling() && engine.stage() == "drawing") {
        assert_eq!(engine.step(0.0), MORE, "the spacing problem returned");
    }
    assert_eq!(engine.stage(), "drawing");
    engine.put(FILE, "/s/a.ly", Some(last.as_bytes()));
    assert!(!engine.calling(), "a newer file drops the call");
    compiled(&mut engine, &last);
    assert_eq!(engine.outputs(), expected, "stopped in a call");

    // Stopped twice, the second before the first's text was ever finished.
    compiled(&mut engine, &text);
    for (k, edit) in [&stopped, &last].into_iter().enumerate() {
        engine.put(FILE, "/s/a.ly", Some(edit.as_bytes()));
        engine.compile(2 + k as u32, "/s/a.ly");
        for _ in 0..(total / 2) {
            assert_eq!(engine.step(0.0), MORE);
        }
    }
    compiled(&mut engine, &text);
    assert_eq!(
        engine.outputs(),
        whole(&text),
        "stopped twice, then the first text"
    );
    assert!(engine.resumed_at().iter().any(Option::is_some));
}

/// A collection goes in steps of its own: what is held between a compile's
/// steps is rooted, whenever it comes, and when it comes changes no output
/// (B20, D261). A newer compile drops one halfway.
#[test]
fn a_collection_goes_between_a_compiles_steps_and_a_newer_one_drops_it() {
    let _alone = alone();
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let datadir = lily.join("../..");
    let fresh = || {
        let mut engine =
            engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
        engine.set_returns(true);
        engine
    };
    let text = score("c'8 d' e' f' g' a' b' c''");
    let edit = text.replacen("d'1 e'1", "d'1 g'1", 1);
    let whole = |text: &str| {
        let mut engine = fresh();
        engine.set_returns(false);
        engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
        answered(&mut engine, "/s/a.ly", &datadir, 0.0);
        engine.outputs().to_vec()
    };
    let (first, second) = (whole(&text), whole(&edit));

    // A collection at the start of every step of a compile, an edit too.
    let mut engine = fresh();
    engine.set_collect_every_step(true);
    engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
    answered(&mut engine, "/s/a.ly", &datadir, 20.0);
    assert_eq!(engine.outputs(), first);
    engine.put(FILE, "/s/a.ly", Some(edit.as_bytes()));
    answered(&mut engine, "/s/a.ly", &datadir, 20.0);
    assert_eq!(engine.outputs(), second);

    // With no time, a collection is many steps; stopped in the middle of one
    // of an edit's, what it marked of what stays is not taken for marked by the
    // next: a collection at each of that compile's steps must find it.
    let mut engine = fresh();
    engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
    answered(&mut engine, "/s/a.ly", &datadir, 0.0);
    let middle = text.replacen("d'1 e'1", "d'1 a'1", 1);
    engine.put(FILE, "/s/a.ly", Some(middle.as_bytes()));
    engine.compile(2, "/s/a.ly");
    let mut slices = 0;
    while slices < 4 {
        assert_eq!(engine.step(0.0), MORE, "the compile saw a collection");
        slices += usize::from(engine.collecting());
    }
    engine.put(FILE, "/s/a.ly", Some(edit.as_bytes()));
    assert!(!engine.collecting(), "a newer file drops the collection");
    engine.set_collect_every_step(true);
    answered(&mut engine, "/s/a.ly", &datadir, 20.0);
    assert_eq!(engine.outputs(), second);
}

// D261's guards that the tests above pass without (OPEN B30), each tested
// alone: a guard taken out fails its test (the tracing of `given` only with
// debug assertions, as its test says).

/// A fresh engine of the image, returning or not.
fn fresh(lily: &Path, returns: bool) -> Engine {
    let mut engine = engine_of(support::library_with(lily, lily_engine_core::install).unwrap());
    engine.set_returns(returns);
    engine
}

/// The outputs of a fresh engine's compile of `text` in one call.
fn whole(lily: &Path, text: &str) -> Vec<(String, Vec<u8>)> {
    let mut engine = fresh(lily, false);
    engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
    answered(&mut engine, "/s/a.ly", &lily.join("../.."), 0.0);
    engine.outputs().to_vec()
}

/// A translation that ended as a step's time ran out is rooted until the next
/// step lays it out (`Stage::Ended`, D261): in a resumed compile nothing else
/// reaches it, and a collection there would free it (OPEN B30).
#[test]
fn a_translation_ended_at_a_step_s_end_lives_through_a_collection_there() {
    let _alone = alone();
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let datadir = lily.join("../..");
    let text = score("c'4 d' e' f'");
    let edit = text.replacen("d'1 e'1", "d'2 d' e'1", 1);
    let mut engine = fresh(&lily, true);
    for t in [&text, &edit] {
        engine.put(FILE, "/s/a.ly", Some(t.as_bytes()));
        let mut collected = 0;
        answered_with(&mut engine, "/s/a.ly", &datadir, 0.0, |engine| {
            if engine.stage() == "ended" {
                engine.collect();
                collected += 1;
            }
        });
        assert!(collected > 0, "no step ended with a translation ended");
        assert_eq!(engine.outputs(), whole(&lily, t));
    }
    assert!(engine.resumed_at().iter().all(Option::is_some));
}

/// What a resume gives back of the kept translations is pictured (`given`,
/// D261), and a stop puts it back: the collector traces those pictures, which
/// hold what the last compile's layout wrote to the old run's grobs after its
/// end, and nothing else holds (OPEN B30). Without them the kept translations
/// would hold freed objects after the stop, which no later compile reads: each
/// resume gives those grobs back again before its layout. So what fails
/// without the tracing is `Arena::get`'s debug check, where the collections
/// after the stop mark what the kept translations hold: in a build with debug
/// assertions (`cargo test` without `--release`); in release this checks the
/// outputs alone.
#[test]
fn what_a_stopped_resume_gave_back_lives_through_its_collections() {
    let _alone = alone();
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let datadir = lily.join("../..");
    let text = score("c'4 d' e' f'");
    // The bytes after it moved: the second compile resumes, splices, and lays
    // out the old run's grobs again, which its pictures keep.
    let spaced = text.replacen("d'1 e'1", "d'1  e'1", 1);
    let edit = spaced.replacen("d'1  e'1", "d'1  g'1", 1);
    let mut engine = fresh(&lily, true);
    for t in [&text, &spaced] {
        engine.put(FILE, "/s/a.ly", Some(t.as_bytes()));
        answered(&mut engine, "/s/a.ly", &datadir, 20.0);
    }
    assert!(
        engine.spliced().iter().all(|&s| s),
        "{:?}",
        engine.spliced()
    );
    // Stopped once its translation has resumed, a collection at each step.
    engine.set_collect_every_step(true);
    engine.put(FILE, "/s/a.ly", Some(edit.as_bytes()));
    engine.compile(2, "/s/a.ly");
    let mut steps = 0;
    while engine.stage() != "translating" || steps < 3 {
        assert_eq!(engine.step(0.0), MORE, "the compile ran on");
        steps += usize::from(engine.stage() == "translating");
    }
    engine.put(FILE, "/s/a.ly", Some(spaced.as_bytes()));
    assert!(!engine.running(), "a newer file stops it");
    // What the stop put back is marked, then read by the same text's resume.
    engine.collect();
    answered(&mut engine, "/s/a.ly", &datadir, 20.0);
    engine.set_collect_every_step(false);
    assert_eq!(engine.outputs(), whole(&lily, &spaced));
    assert!(engine.resumed_at().iter().all(Option::is_some));
}

/// A splice takes the old run's records from where the runs meet to its end
/// (D243), the links to places in its playback map among them: they are
/// moved to the edited text in a copy of the old run's end (D244, D261),
/// which a stop leaves of the old text (OPEN B30).
#[test]
fn a_splice_moves_the_old_run_s_end_to_the_edited_text() {
    let _alone = alone();
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let datadir = lily.join("../..");
    let text = score("c'4 d' e' f'");
    // The same music, every byte after the edit moved by one.
    let spaced = text.replacen("d'1 e'1", "d'1  e'1", 1);
    let mut engine = fresh(&lily, true);
    for t in [&text, &spaced] {
        engine.put(FILE, "/s/a.ly", Some(t.as_bytes()));
        answered(&mut engine, "/s/a.ly", &datadir, 0.0);
    }
    assert!(engine.resumed_at().iter().all(Option::is_some));
    assert!(
        engine.spliced().iter().all(|&s| s),
        "{:?}",
        engine.spliced()
    );
    let expected = whole(&lily, &spaced);
    assert!(expected.iter().any(|(n, _)| n.ends_with(".timing.json")));
    assert_eq!(engine.outputs(), expected);
}

/// Near the memory's cap, within `DEFERRING_ROOM`, a compile in steps collects
/// at safe points as it did before D261, not between its steps: a collection
/// that waits for a step's end grows the arenas there (OPEN B30).
#[test]
fn near_the_cap_a_compile_collects_as_safe_points_come() {
    let _alone = alone();
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let datadir = lily.join("../..");
    /// The process's memory check put back, whatever the test does.
    struct Roomy;
    impl Drop for Roomy {
        fn drop(&mut self) {
            lily_engine_music::returns::set_room(|_| false);
        }
    }
    let _roomy = Roomy;
    let text = score("c'8 d' e' f' g' a' b' c''");
    let expected = whole(&lily, &text);
    // How many of a compile's returns found a collection under way, and the
    // collections it made.
    let collecting = |engine: &mut Engine| {
        let before = engine.collect().collections;
        engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
        let mut under_way = 0;
        answered_with(engine, "/s/a.ly", &datadir, 0.0, |engine| {
            under_way += usize::from(engine.collecting());
        });
        assert_eq!(engine.outputs(), expected);
        (under_way, engine.collect().collections - before - 1)
    };
    let mut engine = fresh(&lily, true);
    let (far, _) = collecting(&mut engine);
    assert!(far > 0, "far from the cap, a collection goes between steps");
    // 16 MiB left: less than `DEFERRING_ROOM`, more than any one grob asks.
    let mut engine = fresh(&lily, true);
    engine.set_room(|extra| extra >= 16 << 20);
    let (near, collections) = collecting(&mut engine);
    assert_eq!(
        near, 0,
        "near the cap, no collection waits for a step's end"
    );
    assert!(collections > 0, "it collected at safe points");
}

/// A toplevel score of `n` staves, each `\repeat unfold` of `notes`: its
/// `scorify-music` makes the copies, and so does its translation's start.
fn unfolded(staves: usize, times: usize, notes: &str) -> String {
    let staff = format!("    \\new Staff {{ \\repeat unfold {times} {{ {notes} | }} }}\n");
    format!(
        "\\version \"2.26.0\"\n\\score {{\n  <<\n{}  >>\n  \\layout {{ }}\n}}\n",
        staff.repeat(staves)
    )
}

/// A `\score`'s `scorify-music` returns in its steps (B23, D272): the parse
/// stops in the score, its call under way in the VM, and goes on there; a
/// newer edit stops it wherever it is, and nothing of it shows after.
#[test]
fn a_score_s_scorify_music_returns_in_its_steps_and_a_newer_edit_stops_it() {
    let _alone = alone();
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let datadir = lily.join("../..");
    let text = unfolded(2, 120, "c'8 d' e' f' g' a' b' c''");
    let other = unfolded(2, 60, "e'4 f' g' a'");
    let one_call = whole(&lily, &text);

    let mut stepped = fresh(&lily, true);
    stepped.put(FILE, "/s/a.ly", Some(text.as_bytes()));
    let mut in_call = 0;
    answered_with(&mut stepped, "/s/a.ly", &datadir, 0.0, |e| {
        if e.stage() == "parsing" && e.calling() {
            in_call += 1;
        }
    });
    assert!(
        in_call >= 2,
        "the parse returned {in_call} times in the score's call"
    );
    assert_eq!(stepped.outputs(), one_call);

    // Stopped in the call early and late, by another text and by the same,
    // in an engine that has its packs.
    let warm = unfolded(1, 2, "c'1");
    // Each step of a budget of 0 returns at the VM's next tick check: 76 in
    // this call.
    for at in [1, 20] {
        for newer in [&other, &text] {
            let mut engine = fresh(&lily, true);
            engine.put(FILE, "/s/a.ly", Some(warm.as_bytes()));
            answered(&mut engine, "/s/a.ly", &datadir, 0.0);
            engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
            engine.compile(2, "/s/a.ly");
            let mut seen = 0;
            while seen < at {
                assert_eq!(engine.step(0.0), MORE);
                if engine.calling() {
                    seen += 1;
                }
            }
            assert_eq!(engine.stage(), "parsing");
            engine.put(FILE, "/s/a.ly", Some(newer.as_bytes()));
            assert!(
                !engine.running() && !engine.calling(),
                "a newer file stops it"
            );
            answered(&mut engine, "/s/a.ly", &datadir, 0.0);
            assert_eq!(engine.outputs(), whole(&lily, newer), "stopped at {at}");
        }
    }
}

/// A translation's start asks ahead, a music's list a step, for what its
/// `create_top` reads (B23, D272), and writes what it wrote; a newer edit
/// stops it there.
#[test]
fn a_translation_s_start_asks_its_repeats_ahead_in_steps() {
    let _alone = alone();
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let datadir = lily.join("../..");
    let text = unfolded(6, 8, "c'4 d' e' f'");
    let other = unfolded(6, 8, "e'4 f' g' a'");
    let one_call = whole(&lily, &text);
    let before = lily_engine_music::returns::returns_ahead();
    let mut stepped = fresh(&lily, true);
    stepped.put(FILE, "/s/a.ly", Some(text.as_bytes()));
    answered(&mut stepped, "/s/a.ly", &datadir, 0.0);
    let ahead = lily_engine_music::returns::returns_ahead() - before;
    assert!(
        ahead >= 5,
        "the translation's start returned {ahead} times asking ahead"
    );
    assert_eq!(stepped.outputs(), one_call);

    let mut engine = fresh(&lily, true);
    engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
    engine.compile(1, "/s/a.ly");
    let before = lily_engine_music::returns::returns_ahead();
    while lily_engine_music::returns::returns_ahead() < before + 3 {
        assert_eq!(engine.step(0.0), MORE);
    }
    engine.put(FILE, "/s/a.ly", Some(other.as_bytes()));
    answered(&mut engine, "/s/a.ly", &datadir, 0.0);
    assert_eq!(engine.outputs(), whole(&lily, &other));
}

/// Two lines of marks above staves, as T29's MarkLine and SmallMarkLine:
/// each one's metronome mark reads the lines below it through the
/// alignment's translations.
fn marked() -> String {
    let notes = "\\repeat unfold 6 { c''4\\f d'' e'' f''^\\markup \\italic \"dolce\" | }";
    let staff = format!("    \\new Staff {{ {notes} }}\n");
    format!(
        "\\version \"2.26.0\"
\\layout {{
  \\context {{
    \\type Engraver_group
    \\name MarkLine
    \\consists Axis_group_engraver
    \\consists Mark_engraver
    \\consists Metronome_mark_engraver
    \\consists Text_engraver
    \\override VerticalAxisGroup.staff-affinity = #DOWN
    \\override VerticalAxisGroup.nonstaff-relatedstaff-spacing.padding = #2
  }}
  \\context {{ \\Score \\accepts MarkLine }}
}}
marks = {{ \\tempo \"Presto\" 4 = 132 s1*3 \\mark \\default s1*3 }}
\\score {{
  <<
    \\new MarkLine {{ \\marks }}
{staff}{staff}    \\new MarkLine {{ \\marks }}
{staff}{staff}  >>
}}
"
    )
}

/// A line of marks is held before its first outside-staff measure while the
/// lines below it are read in steps of their own (B28, D272), and what the
/// nested read LilyPond makes there would have asked before them is made
/// again: every output as the nested read's, in steps and whole.
#[test]
fn a_line_of_marks_is_held_while_the_lines_below_it_are_read() {
    let _alone = alone();
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let datadir = lily.join("../..");
    let text = marked();
    lily_engine_layout::system::set_holding(false);
    let nested = whole(&lily, &text);
    lily_engine_layout::system::set_holding(true);
    let before = lily_engine_layout::system::held();
    let held = whole(&lily, &text);
    // Each line of marks, in each compile the packs it asks for start again.
    let n = lily_engine_layout::system::held() - before;
    assert!(n >= 2, "{n} lines held");
    assert_eq!(held, nested);
    let mut stepped = fresh(&lily, true);
    stepped.put(FILE, "/s/a.ly", Some(text.as_bytes()));
    answered(&mut stepped, "/s/a.ly", &datadir, 0.0);
    assert_eq!(stepped.outputs(), nested);

    // T29, whose SmallMarkLine's own nested read comes inside its MarkLine's:
    // held, the MarkLine's translations and the SmallMarkLine's first
    // part-of-line height are made again, as LilyPond's nested read makes
    // them after the inner one (its `\moltocr` dash moved without them, as
    // `compile`'s outputs have it against LilyPond's).
    let Some(t29) = corpus_score("T29") else {
        eprintln!("skipped T29: no $LILY_CORPUS/corpus.tsv");
        return;
    };
    let text = std::fs::read_to_string(t29).unwrap();
    lily_engine_layout::system::set_holding(false);
    let nested = whole(&lily, &text);
    lily_engine_layout::system::set_holding(true);
    let held = whole(&lily, &text);
    let dash = |o: &[(String, Vec<u8>)]| {
        o.iter().filter(|(n, _)| n.ends_with(".svg")).any(|(_, b)| {
            String::from_utf8_lossy(b).contains("stroke-dasharray=\"0,2.9384833592448927\"")
        })
    };
    assert!(dash(&held), "T29's `\\moltocr` dash is LilyPond's");
    assert!(held == nested, "T29 held is not T29 nested");
}

/// The path of the corpus score `id` in `$LILY_CORPUS/corpus.tsv` (by default
/// `~/.cache/lily-engine`), as `engine.rs` reads it.
fn corpus_score(id: &str) -> Option<std::path::PathBuf> {
    let root = std::env::var_os("LILY_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".cache/lily-engine")
        });
    let tsv = std::fs::read_to_string(root.join("corpus.tsv")).ok()?;
    tsv.lines()
        .filter_map(|line| line.split_once('\t'))
        .find(|(i, _)| *i == id)
        .map(|(_, path)| std::path::PathBuf::from(path))
}

/// The global loop returns in the middle of a timestep, after its checkpoint
/// and after its iterators' events (B23, D272), and writes what the one call
/// writes; a newer edit stops it there.
#[test]
fn a_timestep_returns_after_its_checkpoint_and_its_events() {
    let _alone = alone();
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let datadir = lily.join("../..");
    let text = unfolded(4, 4, "c'4 d' e' f'");
    let other = unfolded(4, 4, "e'4 f' g' a'");
    let one_call = whole(&lily, &text);
    let before = lily_engine_music::returns::returns_mid();
    let mut stepped = fresh(&lily, true);
    stepped.put(FILE, "/s/a.ly", Some(text.as_bytes()));
    answered(&mut stepped, "/s/a.ly", &datadir, 0.0);
    let mid = lily_engine_music::returns::returns_mid() - before;
    assert!(mid >= 16, "{mid} returns in a timestep");
    assert_eq!(stepped.outputs(), one_call);

    for at in [1, 7, 20] {
        let mut engine = fresh(&lily, true);
        engine.put(FILE, "/s/a.ly", Some(other.as_bytes()));
        answered(&mut engine, "/s/a.ly", &datadir, 0.0);
        engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
        engine.compile(2, "/s/a.ly");
        let before = lily_engine_music::returns::returns_mid();
        while lily_engine_music::returns::returns_mid() < before + at {
            assert_eq!(engine.step(0.0), MORE);
        }
        engine.put(FILE, "/s/a.ly", Some(other.as_bytes()));
        answered(&mut engine, "/s/a.ly", &datadir, 0.0);
        assert_eq!(engine.outputs(), whole(&lily, &other), "stopped at {at}");
    }
}
