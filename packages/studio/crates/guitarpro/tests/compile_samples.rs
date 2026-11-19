//! The seven samples of README §1 read and written as LilyPond, and each
//! score compiled with lilypond: no error, and no warning of the writer's
//! making. The samples are copyrighted songs and so not in the repository:
//! they are read from `GUITAR_PRO_SAMPLES` (unset, none
//! is found), and lilypond is `LILYPOND_PATH`, else the
//! PATH, else Homebrew's. The test skips what it cannot find and says so; a
//! run that skipped has not checked the writer. `GUITAR_PRO_OUT` keeps the
//! scores and what lilypond made of them in that directory, which must stay
//! out of the repository, as the songs do.
use std::path::{Path, PathBuf};
use std::process::Command;

use lily_guitarpro::{lilypond, read};

const SAMPLES: [&str; 7] = [
    "gpx-a.gpx",
    "gp4-a.gp4",
    "gp3-b.gp3",
    "gp3-a.gp3",
    "gp5-b.gp5",
    "gp5-c.gp5",
    "gp5-a.gp5",
];

fn samples() -> PathBuf {
    std::env::var_os("GUITAR_PRO_SAMPLES").map_or_else(PathBuf::new, PathBuf::from)
}

fn lilypond_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("LILYPOND_PATH") {
        return Some(PathBuf::from(path));
    }
    let on_path = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join("lilypond"))
            .find(|p| p.is_file())
    });
    on_path.or_else(|| {
        let homebrew = Path::new("/opt/homebrew/bin/lilypond");
        homebrew.is_file().then(|| homebrew.to_path_buf())
    })
}

/// What lilypond says that is not about our output: none so far.
const ALLOWED: [&str; 0] = [];

#[test]
fn the_samples_compile_cleanly() {
    let Some(lilypond) = lilypond_path() else {
        eprintln!("skipped: lilypond is not installed");
        return;
    };
    let kept = std::env::var_os("GUITAR_PRO_OUT").map(PathBuf::from);
    let out = kept.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("lily-guitarpro-samples-{}", std::process::id()))
    });
    std::fs::create_dir_all(&out).unwrap();
    let mut compiled = 0;
    let mut failures = Vec::new();
    for name in SAMPLES {
        let source = samples().join(name);
        let Ok(bytes) = std::fs::read(&source) else {
            eprintln!("skipped {}: not found", source.display());
            continue;
        };
        let extension = name.rsplit('.').next();
        let song = read(&bytes, extension).unwrap_or_else(|e| panic!("{name}: {e}"));
        let stem = name.rsplit_once('.').unwrap().0;
        let file = out.join(format!("{stem}.ly"));
        std::fs::write(&file, lilypond::write(&song)).unwrap();
        let output = Command::new(&lilypond)
            .arg("-dno-point-and-click")
            .arg("-o")
            .arg(out.join(stem))
            .arg(&file)
            .output()
            .expect("lilypond runs");
        let log = String::from_utf8_lossy(&output.stderr).to_string();
        let problems: Vec<&str> = log
            .lines()
            .filter(|l| l.contains("error:") || l.contains("warning:"))
            .filter(|l| l.contains("error:") || !ALLOWED.iter().any(|a| l.contains(a)))
            .collect();
        let midi = out.join(format!("{stem}.midi")).is_file()
            || out.join(format!("{stem}-1.midi")).is_file();
        if !output.status.success() || !problems.is_empty() || !midi {
            failures.push(format!(
                "{name}: exit {:?}, {} problems, MIDI {midi}\n{}",
                output.status.code(),
                problems.len(),
                problems.join("\n")
            ));
        } else {
            eprintln!("compiled {name} cleanly");
        }
        compiled += 1;
    }
    if kept.is_none() {
        let _ = std::fs::remove_dir_all(&out);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    if compiled == 0 {
        eprintln!("skipped: no samples in {}", samples().display());
    } else {
        eprintln!("compiled {compiled} samples with {}", lilypond.display());
    }
}
