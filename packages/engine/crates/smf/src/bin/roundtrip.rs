//! roundtrip [file.midi…]: each of LilyPond's files read back into the staves
//! of audio items it was written from, written again, and compared byte for
//! byte (07 §4.2). A file that differs only in the order of note-offs at one
//! tick is told apart from one that differs in anything else, which is a bug.
//! Without files, every `.midi` of `$LILY_CORPUS/ref` (default
//! `~/.cache/lily-engine`), which `npm run capture` writes. With `--out <dir>`,
//! each file written goes to `<dir>/<its directory's name>/`, for score.mjs's
//! C2b against the references.
//!
//!   cargo +stable run --release --bin roundtrip -- [--out <dir>] [file.midi…]
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use lily_engine_smf::{events, read, sort_tied_offs, write};

fn corpus() -> Vec<PathBuf> {
    let root = std::env::var_os("LILY_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache/lily-engine")
        });
    let mut files: Vec<PathBuf> = std::fs::read_dir(root.join("ref"))
        .into_iter()
        .flatten()
        .flatten()
        .flat_map(|id| std::fs::read_dir(id.path()).into_iter().flatten().flatten())
        .map(|f| f.path())
        .filter(|f| f.extension().is_some_and(|e| e == "midi"))
        .collect();
    files.sort();
    files
}

/// The note-offs written in another order, and the ticks they are at.
fn tied(ours: &[u8], theirs: &[u8]) -> (usize, usize) {
    let (Ok(a), Ok(b)) = (
        lily_engine_smf::chunks(ours),
        lily_engine_smf::chunks(theirs),
    ) else {
        return (0, 0);
    };
    let (mut offs, mut ticks) = (0, Vec::new());
    for (a, b) in a.iter().zip(&b).filter(|(a, _)| &a.tag == b"MTrk") {
        let (Ok(a), Ok(b)) = (events(a.body()), events(b.body())) else {
            continue;
        };
        for (x, y) in a.iter().zip(&b).filter(|(x, y)| x.bytes != y.bytes) {
            offs += 1;
            ticks.push((x.tick, y.tick));
        }
    }
    ticks.dedup();
    (offs, ticks.len())
}

fn main() -> ExitCode {
    let mut args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let out_dir = match args.iter().position(|a| a.as_os_str() == "--out") {
        Some(at) if at + 1 < args.len() => Some(args.drain(at..at + 2).nth(1).unwrap()),
        Some(_) => {
            eprintln!("roundtrip: --out needs a directory");
            return ExitCode::from(2);
        }
        None => None,
    };
    let files = if args.is_empty() { corpus() } else { args };
    if files.is_empty() {
        eprintln!(
            "roundtrip: no files, and no captures in $LILY_CORPUS/ref (npm run corpus && npm run capture)"
        );
        return ExitCode::from(2);
    }
    let (mut same, mut ties, mut differ) = (0, 0, 0);
    for path in &files {
        let name = path.display();
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) => {
                println!("DIFF\t{name}\tcannot read: {e}");
                differ += 1;
                continue;
            }
        };
        let staves = match read(&bytes) {
            Ok(staves) => staves,
            Err(e) => {
                println!("DIFF\t{name}\tcannot parse: {e:?}");
                differ += 1;
                continue;
            }
        };
        let mut out = Vec::new();
        let t0 = Instant::now();
        write(&staves, &mut out);
        let us = t0.elapsed().as_micros();
        if let Some(dir) = &out_dir {
            let dir = dir.join(
                path.parent()
                    .and_then(|p| p.file_name())
                    .unwrap_or_default(),
            );
            let written = std::fs::create_dir_all(&dir).and_then(|()| {
                std::fs::write(dir.join(path.file_name().unwrap_or_default()), &out)
            });
            if let Err(e) = written {
                eprintln!("roundtrip: cannot write into {}: {e}", dir.display());
                return ExitCode::from(2);
            }
        }
        if out == bytes {
            same += 1;
            println!("same\t{name}\t{} B\t{us} µs", bytes.len());
        } else if sort_tied_offs(&out) == sort_tied_offs(&bytes) {
            ties += 1;
            let (offs, ticks) = tied(&out, &bytes);
            println!(
                "ties\t{name}\t{offs} note-offs at {ticks} tick{} in another order",
                if ticks == 1 { "" } else { "s" }
            );
        } else {
            differ += 1;
            let first = out.iter().zip(&bytes).position(|(a, b)| a != b);
            println!(
                "DIFF\t{name}\t{} B against {} B, first at byte {first:?}",
                out.len(),
                bytes.len()
            );
        }
    }
    println!(
        "{same} of {} byte for byte; {ties} equal with the note-offs at one tick sorted; {differ} differ",
        files.len()
    );
    if differ > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
