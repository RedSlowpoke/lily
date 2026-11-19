//! Parses .ly files and prints, for each, what LilyPond prints to stderr for
//! it with `--loglevel=WARNING`, as far as the parser goes (P1 to P5's
//! music functions so far). The file's name in locations is the path as
//! given, as lilypond prints it.
//!
//!   cargo +stable run -p lily-engine-parser --example parse -- \
//!     [--lily <scm/lily>] [--note-names <define-note-names.scm>] [--out <dir>] [--tokens <dir>] [--music <dir>] [--display <dir>] [--time] <file.ly>…
//!
//! `\name` is classified in a module that uses `(lily)` of one VM, which
//! loaded `lily.scm`'s core files, with what the parser needs besides and
//! the stubs of the parts on demand, then parsed and ran
//! `ly/declarations-init.ly` and the files it includes (`tests/support`'s
//! `load_library`), from `--lily`, or else from the installed LilyPond
//! 2.26.0's `scm/lily/`, where a part's files are read when one of its
//! stubs is called (D74); each file gets a module of its own, which uses
//! the init file's, and its session ends after it (`session-terminate`). The pitch names are those of `--note-names`, or else
//! of `define-note-names.scm` there. Without LilyPond's files, the runner
//! refuses to run.
//! With `--out`, each file's stderr goes to `<dir>/<name>.err` instead, the
//! layout `tools/compare/score.mjs --errors` reads. With `--tokens`, the
//! tokens the parser read go to `<dir>/<name>.tokens`, one name a line, as
//! `tools/compare/tokens.mjs` writes LilyPond's from `-ddebug-parser`'s
//! trace and compares them. With `--music`, the music of its scores goes to
//! `<dir>/<name>.music` in `tools/compare/music.ly`'s format (`tests/support`'s
//! `music_trees`), which `tools/compare/music.mjs` compares. With `--display`,
//! what its Scheme wrote to the standard output port, which lilypond prints
//! to stdout, goes to `<dir>/<name>.out` (`\displayMusic`'s, for
//! `tools/bench/probe-functions.ly`). `\include "file"` reads the file from
//! disk, as `crates/core/examples/compile.rs` answers it (MS2 step 20): a
//! name under `<datadir>/` from LilyPond's data directory (`--lily`'s
//! `../..`), any other relative to the current directory. With `--time`, each
//! file's name and the milliseconds its parse and stderr took go to stdout,
//! a tab between. Exits with 1 when a file failed, as lilypond does.

use lily_engine_parser::NoteNames;
use lily_engine_scheme::Hosted;
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::{env, fs, io::Write, process};

#[path = "../tests/support/mod.rs"]
mod support;

/// The value of `--flag`, taken out of `args`.
fn option(args: &mut Vec<String>, flag: &str) -> Option<String> {
    match args.iter().position(|a| a == flag) {
        Some(i) if i + 1 < args.len() => Some(args.drain(i..i + 2).nth(1).unwrap_or_default()),
        Some(_) => usage(),
        None => None,
    }
}

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let out = option(&mut args, "--out");
    let tokens = option(&mut args, "--tokens");
    let music = option(&mut args, "--music");
    let display = option(&mut args, "--display");
    let flag = |args: &mut Vec<String>, flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .map(|i| args.remove(i))
            .is_some()
    };
    let time = flag(&mut args, "--time");
    let lily = option(&mut args, "--lily")
        .map(PathBuf::from)
        .or_else(support::lily_scm)
        .unwrap_or_else(|| {
            eprintln!("parse: no scm/lily: install LilyPond 2.26.0 or pass --lily");
            process::exit(2)
        });
    let names_file = option(&mut args, "--note-names")
        .map(PathBuf::from)
        .unwrap_or_else(|| lily.join("define-note-names.scm"));
    let names = fs::read(&names_file)
        .map_err(|e| e.to_string())
        .and_then(|text| NoteNames::read(&text))
        .unwrap_or_else(|e| {
            eprintln!("parse: {}: {e}", names_file.display());
            process::exit(2)
        });
    if args.is_empty() {
        usage();
    }
    let mut vm = support::library(&lily).unwrap_or_else(|e| {
        eprintln!("parse: {}: {e}", lily.display());
        process::exit(2)
    });
    let mut failed = false;
    for file in &args {
        let text = fs::read(file).unwrap_or_else(|e| {
            eprintln!("parse: {file}: {e}");
            process::exit(2)
        });
        let datadir = lily.join("../..");
        let mut include = |name: &str| {
            let path = match name.strip_prefix(lily_engine_parser::source::DATADIR) {
                Some(rest) => datadir.join(rest.trim_start_matches('/')),
                None => PathBuf::from(name),
            };
            fs::read(path).map_or(Hosted::Missing, Hosted::Text)
        };
        let start = Instant::now();
        let parse = match tokens {
            Some(_) => lily_engine_parser::parse_tokens_including(
                file,
                text,
                &names,
                &mut vm,
                &mut include,
            ),
            None => lily_engine_parser::parse_including(file, text, &names, &mut vm, &mut include),
        };
        failed |= parse.failed();
        let stderr = parse.stderr();
        let output = vm.take_output();
        if let Some(dir) = &display {
            write(dir, file, "out", output.as_bytes());
        }
        if time {
            println!("{file}\t{:.3}", start.elapsed().as_secs_f64() * 1000.0);
        }
        if let (Some(dir), Some(names)) = (&tokens, &parse.tokens) {
            let lines: String = names.iter().map(|name| format!("{name}\n")).collect();
            write(dir, file, "tokens", lines.as_bytes());
        }
        if let Some(dir) = &music {
            let trees = support::music_trees(&mut vm, &parse).unwrap_or_else(|e| {
                eprintln!("parse: {file}: {e}");
                process::exit(2)
            });
            write(dir, file, "music", trees.as_bytes());
        }
        match &out {
            Some(dir) => write(dir, file, "err", &stderr),
            None => {
                let _ = std::io::stderr().write_all(&stderr);
            }
        }
        // As `lilypond-all` ends each file's session.
        lily_engine_parser::end_session(&mut vm);
    }
    process::exit(i32::from(failed));
}

/// Writes `bytes` to `<dir>/<file's stem>.<extension>`.
fn write(dir: &str, file: &str, extension: &str, bytes: &[u8]) {
    let stem = Path::new(file).file_stem().unwrap_or_default();
    let path = Path::new(dir).join(format!("{}.{extension}", stem.to_string_lossy()));
    fs::write(&path, bytes).unwrap_or_else(|e| {
        eprintln!("parse: {}: {e}", path.display());
        process::exit(2)
    });
}

fn usage() -> ! {
    eprintln!(
        "usage: parse [--lily <scm/lily>] [--note-names <define-note-names.scm>] [--out <dir>] [--tokens <dir>] [--music <dir>] [--display <dir>] [--time] <file.ly>…"
    );
    process::exit(2)
}
