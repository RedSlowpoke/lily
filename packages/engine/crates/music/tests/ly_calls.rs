//! N1 (chapter 04 §13): the `ly:` functions of moments, durations, pitches
//! and scales against LilyPond 2.26.0. `tools/compare/n1-calls.ly` lists the
//! calls and prints what LilyPond gives for each; the engine runs the same
//! calls, read from that file, in turn in one VM's `(lily)`, and must give
//! the same: the value as `write` writes it, or the error's key and text.
//! Skipped without LilyPond 2.26.0, and then unverified.
//!
//! The checked arithmetic, where LilyPond's wraps, is the engine's alone.
use std::path::PathBuf;
use std::process::Command;

use lily_engine_scheme::Vm;

/// The capture, beside the engine's other LilyPond tools.
fn capture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/compare/n1-calls.ly")
}

/// The calls of `n1-calls.ly`: one per line of its list.
fn calls() -> Vec<String> {
    let text = std::fs::read_to_string(capture()).unwrap();
    let start = text.find("#(define n1-calls '(\n").unwrap();
    let list = &text[start..];
    let end = list.find("\n))\n").unwrap();
    list[..end]
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect()
}

/// A VM with the music crate's functions, `(lily)` current.
fn vm() -> Vm {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    vm.eval_text("(set-current-module (resolve-module '(lily)))", None)
        .unwrap();
    vm
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

/// What LilyPond 2.26.0 gives for each call, in order; `None` without it.
fn lilypond() -> Option<Vec<(String, String)>> {
    let version = Command::new("lilypond").arg("--version").output().ok()?;
    if !String::from_utf8_lossy(&version.stdout).contains("GNU LilyPond 2.26.0") {
        return None;
    }
    let dir = std::env::temp_dir().join(format!("lily-engine-n1-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new("lilypond")
        .arg("-s")
        .arg(capture())
        .current_dir(&dir)
        .output()
        .unwrap();
    std::fs::remove_dir_all(&dir).ok();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let mut records: Vec<&str> = stdout.split('\u{1e}').collect();
    records.pop();
    Some(
        records
            .into_iter()
            .map(|r| {
                let (key, text) = r.split_once('\t').unwrap_or((r, ""));
                (key.to_string(), text.trim_end_matches('\n').to_string())
            })
            .collect(),
    )
}

#[test]
fn the_calls_give_what_lilypond_gives() {
    let calls = calls();
    let Some(expected) = lilypond() else {
        eprintln!("LilyPond 2.26.0 is not installed: the calls are unverified against it");
        return;
    };
    assert_eq!(expected.len(), calls.len(), "LilyPond ran every call");
    let mut vm = vm();
    let mut failures = Vec::new();
    for (call, (key, text)) in calls.iter().zip(&expected) {
        let (got_key, got) = run(&mut vm, call);
        if (&got_key, &got) != (key, text) {
            failures.push(format!(
                "{call}\n  engine   {got_key}: {got}\n  lilypond {key}: {text}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures of {}:\n{}",
        failures.len(),
        calls.len(),
        failures.join("\n")
    );
}

/// What LilyPond computes in 64 bits and lets wrap is an error here (D61).
#[test]
fn arithmetic_that_does_not_fit_throws() {
    let mut vm = vm();
    let overflow = |subr: &str| {
        (
            "numerical-overflow".to_string(),
            format!(
                "Throw to key `numerical-overflow' with args `(\"{subr}\" \"Numerical overflow\" () #f)'."
            ),
        )
    };
    for (source, subr) in [
        (
            "(ly:moment-mul (ly:make-moment (- (expt 2 62) 1)) 4)",
            "ly:moment-mul",
        ),
        (
            "(ly:moment-add (ly:make-moment 1 (- (expt 2 62) 1)) (ly:make-moment 1 (- (expt 2 62) 3)))",
            "ly:moment-add",
        ),
        (
            "(ly:moment-div (ly:make-moment 1 (expt 2 40)) (expt 2 40))",
            "ly:moment-div",
        ),
        (
            "(ly:duration->number (ly:make-duration 2 70))",
            "ly:duration->number",
        ),
        (
            "(ly:duration-compress (ly:make-duration 2 0 1 (expt 2 61)) 1/16)",
            "ly:duration-compress",
        ),
        (
            "(ly:pitch-steps (ly:make-pitch (expt 2 30) 0))",
            "ly:pitch-steps",
        ),
    ] {
        assert_eq!(run(&mut vm, source), overflow(subr), "{source}");
    }
    // Reduced before it is multiplied, a product that fits is exact.
    assert_eq!(
        run(&mut vm, "(ly:moment-mul (ly:make-moment (expt 2 40) (- (expt 2 61) 1)) (/ (- (expt 2 61) 1) (expt 2 40)))").1,
        "#<Mom 1>"
    );
}

/// `ly:set-default-scale` refuses what is not a scale, and the probe of the
/// default scale survives a collection.
#[test]
fn the_default_scale_is_lily_scm_s() {
    let mut vm = vm();
    assert_eq!(
        run(&mut vm, "(ly:set-default-scale 5)"),
        (
            "misc-error".to_string(),
            "note scale expected: 5".to_string()
        )
    );
    run(&mut vm, "(define p (ly:make-pitch 0 3 1/2))");
    vm.collect();
    assert_eq!(
        run(
            &mut vm,
            "(list p (ly:pitch-tones p) (map car lilypond-declarations))"
        )
        .1,
        "(#<Pitch fis' > 3 (default-global-scale))"
    );
}
