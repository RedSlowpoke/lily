//! Compiles .ly files through the core's [`Engine`], as chapter 08 §4.2's
//! `lily_put`, `lily_compile` and `lily_step` do in wasm, one instance for
//! them all, and writes each file's two outputs (08 §6): `stderr` to
//! `<dir>/<name>.err` and `diagnostics.json` to `<dir>/<name>.json`.
//!
//!   cargo +stable run -p lily-engine-core --example compile -- \
//!     [--lily <scm/lily>] [--time] [--trace] [--no-playback] \
//!     [--pack <file.pack>]… [--fonts <dir> | --offline] [--break <when>]… \
//!     [--fresh] [--edited <file.ly>]… [--chain] [--n8] [--no-replay | --replay-all]
//!     [--no-cells] [--check-cells] [--preview] [--perform] [--pdf <dir>] [--step-ms <ms>]
//!     [--collect-every <n>]
//!     [--stop-at <steps>] --out <dir> <file.ly>…
//!
//! The engine starts from a core image written here of a VM that loaded the
//! library files and `music-functions-init.ly` from `--lily`, or else from
//! the installed LilyPond 2.26.0's `scm/lily/` (`tests/support`'s
//! `library`, as the parser's runner loads them), whose `(lily)` has the
//! pitch names of `define-note-names.scm`. Each `--pack`, a font pack of
//! W1's (`dist/engine/emmentaler-20.pack`, the packs of
//! `dist/engine/fonts/`), is put first as the module its file is named
//! for, as a host puts the modules it loaded (MS3 step 5). A file is put
//! under its name as given, which its locations print. What the engine
//! asks for (`lily_needs`) is answered from disk, as a host answers: a name
//! under `<datadir>/` from LilyPond's data directory (`--lily`'s `../..`),
//! any other relative to the current directory; one that is not there is
//! put as none. A font pack it asks for (`MODULE`, W5) is answered from
//! `--fonts`, by default the build's `dist/engine/fonts/`, as `<name>.pack`,
//! as a host fetches it, and as none when it is not there or with
//! `--offline`, as a host that cannot fetch it answers. A part's module
//! (I2, [`PART_MODULES`]) is answered with its files from `--lily`, as the
//! build writes it, or as none with `--offline`. Each name is answered as it is asked, and the compile
//! started again, as the studio's driver does; the names one stop asked
//! for, a part's files all at once (MS2 step 30), come one a step with no
//! parse between them. With `--time`, each file's name, the milliseconds
//! from `lily_compile` until `lily_step` returns done, the host's answers
//! included, how many files the engine asked for and at how many stops go
//! to stdout, a tab between: the first file's is a first parse in a fresh
//! engine (MS2 step 20). Then [`Engine::times`]' (MS2 step 27): the last
//! parse's milliseconds, its translations', how many, and the longest's;
//! then (MS3 step 36) the translations' layout and breaking, the pages'
//! stencils and their writing as SVG, in milliseconds, and the live grobs
//! laid out; then (MS5 step 7) the `\midi` translations' part of the
//! translations and the MIDI files' writing; then (MS6 step 3) how many of
//! the last parse's segments were done again from the parse before it and
//! how many were parsed (02 §5.7).
//! With `--trace`, the trace of each file's `\layout` translations in
//! `tools/compare/trace.ly`'s format goes to
//! `<dir>/<name>.trace.tsv`, what they said to
//! `<dir>/<name>.trace.log` (MS2 step 23), the contexts' properties at
//! each bar start that `Timing_translator` finds (MS2 step 25), and the
//! grobs they made, in `tools/compare/grobs.ly`'s format, to
//! `<dir>/<name>.grobs.tsv` (MS3 step 12), and their live grobs, in
//! `tools/compare/layout.ly`'s format, every field after grobs.tsv's six
//! unreached but the Y extent (MS3 step 28), to `<dir>/<name>.layout.tsv`
//! (MS3 step 25), and the springs and rods between their columns, in
//! `layout.ly`'s other format, to `<dir>/<name>.columns.tsv` (MS3 step 31,
//! D128), and the X offsets of their one line's columns, in its third, to
//! `<dir>/<name>.line.tsv` (MS3 step 32, D129), and their shapes, in
//! `tools/compare/c5dump.ly`'s format, to `<dir>/<name>.c5.tsv` (MS4
//! step 6). The
//! playback map of each file's `\midi` performances goes to
//! `<dir>/<name>.timing.json`, as `timing.ly` writes it (MS2 step 26);
//! with `--no-playback` the performances run without it, and a file the
//! score's Scheme writes goes there instead (`timing.ly` included, G8).
//! Each file's pages go to `<dir>/<name>.svg`, or `<dir>/<name>-<n>.svg`
//! when a book has more than one (MS3 step 35), as LilyPond's SVG backend
//! names them.
//! Each `--break`, a column's `when` as grobs.tsv writes it (`"2 0"`), is
//! a break the files' lines break at (`Engine::set_breaks`, MS3 step 60)
//! until the engine chooses its own; a test gives LilyPond's.
//! With `--fresh` each file is compiled in an engine of its own, started
//! from the same image, as a page that has compiled nothing compiles it.
//! With `--edited`, one for each file in order, a file is the text before
//! an edit and the `--edited` file its text after (MS6's differential
//! harness, `tools/compare/differential.mjs`): the file is compiled with
//! its own text and its outputs dropped, then put again under its name with
//! the edited text and compiled again in the same engine, the incremental
//! compile, whose outputs go to `<dir>/<n>/`, n the file's place from 1;
//! with `--fresh` too, only the edited text is compiled, from scratch, the
//! compile the incremental one must equal in every layer (00 §6.2, 05
//! §6.3). Until a layer is incremental the two are both from scratch.
//! With `--chain` too (MS7 step 1), a file the same as the one before it is
//! not compiled with its own text: its edit goes on from the last, as a person
//! types.
//! With `--step-ms` (MS6 step 11, H3) compiles return between their steps
//! at that budget, as in wasm; with `--time` too each step's milliseconds and
//! what the compile goes on with go to `<dir>/<name>.steps.tsv`. With
//! `--collect-steps` a compile in steps collects at each step's start, a test's
//! mode (`Engine::set_collect_every_step`). With `--collect-every <n>` every
//! `n`th procedure entry collects (OPEN H15, D268): a value Rust holds unrooted
//! across a call into Scheme is swept there, a test's mode too. With
//! `--stop-at` each compile of the file after its edit, or the file, first runs
//! that many steps times the file's place from 1, and a newer compile stops it;
//! with `--stop-moved` too that compile's text has a comment line more above
//! the edit, so its text is one the compile that stops it moves from.
//! With `--n8` too (MS6 step 4), each incremental compile writes
//! `<name>.n8.tsv` beside its outputs: a line per translation of what it
//! found against the compile before it (`Engine::changes`: `m_e`, the
//! checkpoint it resumes from and the one it converges at, its timesteps,
//! those run again, the changed columns, and with partitions the timesteps
//! each runs and the translators times timesteps run, without partitions
//! and whole), a `parts` line per translation (its parts' translators, why
//! its partitions are one if they are, and their names; MS6 step 6), then
//! a `wrong` line for each thing its whole run contradicted
//! (`Engine::n8_wrong`). A compile replays no partition (D258); `--replay-all`
//! replays those of every translation that has two (MS6 step 7c), and
//! `--no-replay` is what a compile does anyway, kept for the runs that name it.
//! Each incremental compile writes `<name>.cells.tsv` beside its outputs
//! (MS6 step 8): a line per `\layout` translation of what its layout cells
//! did (`Engine::cell_reports`: cells, those reused, grobs, those reused,
//! cells kept, why others were not, and line breaking's lines, those taken
//! from the compile before and those taken that `--check-cells` filled
//! otherwise, MS6 step 9), then an `uncovered` line for each thing their
//! callbacks read that no range of theirs covers, which `--check-cells`
//! lists (05 §6.3);
//! `--no-cells` lays every compile out whole.
//! With `--preview` (MS6 step 10) the compiles write their pages for a
//! preview, as a compile with a visible page does in wasm: 06 §6.2's preview
//! form, and `<page>.cells` beside a page sent by its changed cells. Each
//! incremental compile then writes `<name>.preview.tsv` beside its outputs (a
//! line per page of `Engine::page_reports`, the markup kept last) and, for
//! each `<page>.cells`, the page it replaces cells of as `<page>.before`.
//! With `--perform` (MS7 step 8, D264) each compile leaves its `\midi`s out,
//! as a page's compile in wasm does, and the performance after it
//! (`Engine::perform`) writes the MIDI files and the playback map beside the
//! page's outputs, and what its translations said as `<name>.perform.err` and
//! `<name>.perform.json`.
//! With `--pdf <dir>` (MS8 step 5, W7, D270) each compile gives each book's
//! PDF display list (`Engine::set_pdf`), written to `<book>.pdf.list`, and
//! its PDF to `<book>.pdf` by `lily_engine_pdf::write`, the text fonts it
//! names read from `<dir>/<PostScript name>.otf`, or that name in lower case
//! as LilyPond's `fonts/otf` names TeX Gyre's (or the build's
//! `dist/engine/`), a font not there leaving its text out;
//! the writer's warnings go to `<book>.pdf.log`.
//! Exits with 1 when a file failed, or an error of a translation, of
//! `Book::process` or of the pages ended its run (MS3 step 35, D132; a
//! `\book` block's too since MS3 step 64, D220), as lilypond does.

use lily_engine_core::{
    DONE, Engine, FILE, MODULE, MORE, NEEDS, PART_MODULES, write_image, write_part,
};
use lily_engine_parser::source::DATADIR;
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::{env, fs, process};

#[allow(dead_code)]
#[path = "../../parser/tests/support/mod.rs"]
mod support;

/// The value of `--flag`, taken out of `args`.
fn option(args: &mut Vec<String>, flag: &str) -> Option<String> {
    match args.iter().position(|a| a == flag) {
        Some(i) if i + 1 < args.len() => Some(args.drain(i..i + 2).nth(1).unwrap_or_default()),
        Some(_) => usage(),
        None => None,
    }
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("compile: {message}");
    process::exit(2)
}

/// Guile's error for a Scheme expression, after which LilyPond's run ends
/// with 1, by `failed files:` or, at `init.ly`, at once (D132).
const GUILE_ERROR: &[u8] = b"error: Guile signaled an error for the expression beginning here";

/// Where a compile's answers come from: LilyPond's files, the font packs
/// and whether they are fetched; and the budget of a step (`--step-ms`).
struct Host {
    lily: PathBuf,
    datadir: PathBuf,
    fonts: PathBuf,
    offline: bool,
    budget: f64,
    /// Each step's milliseconds and what the compile went on to, the
    /// slowest kept: `<dir>/<stem>.steps.tsv`, with `--time` and `--step-ms`.
    steps: std::cell::RefCell<Vec<(f64, &'static str)>>,
}

/// Compiles `file` as revision `revision` until `lily_step` returns done,
/// each name the engine asks for answered from disk and the compile started
/// again; how many files it asked for, and at how many stops.
fn run(engine: &mut Engine, revision: u32, file: &str, host: &Host) -> (usize, usize) {
    run_as(engine, revision, file, host, false)
}

/// [`run`], or with `perform` the performance of the compile (D264), started
/// again the same way after each answer.
fn run_as(
    engine: &mut Engine,
    revision: u32,
    file: &str,
    host: &Host,
    perform: bool,
) -> (usize, usize) {
    let (mut asked, mut stops) = (0, 0);
    // The parses made by the last name asked: a name asked after another
    // parse is a stop of its own.
    let mut parsed = 0;
    let begin = |engine: &mut Engine| match perform {
        true => engine.perform(revision, file),
        false => engine.compile(revision, file),
    };
    begin(engine);
    loop {
        let start = Instant::now();
        let step = engine.step(host.budget);
        host.steps
            .borrow_mut()
            .push((start.elapsed().as_secs_f64() * 1000.0, engine.stage()));
        match step {
            DONE => return (asked, stops),
            NEEDS => {
                let need = engine.needs().map(|(k, n)| (k, n.to_string()));
                if let Some((MODULE, name)) = need {
                    let data = match PART_MODULES.iter().find(|(part, _)| *part == name) {
                        Some((_, files)) => {
                            (!host.offline).then(|| part(&host.lily, files)).flatten()
                        }
                        None => (!host.offline)
                            .then(|| fs::read(host.fonts.join(format!("{name}.pack"))).ok())
                            .flatten(),
                    };
                    engine.put(MODULE, &name, data.as_deref());
                    begin(engine);
                    continue;
                }
                asked += 1;
                if engine.parses() != parsed {
                    (stops, parsed) = (stops + 1, engine.parses());
                }
                let Some((FILE, name)) = need else {
                    fail(format!("{file}: the engine needs {:?}", engine.needs()));
                };
                let path = match name.strip_prefix(DATADIR) {
                    Some(rest) => host.datadir.join(rest.trim_start_matches('/')),
                    None => PathBuf::from(&name),
                };
                engine.put(FILE, &name, fs::read(path).ok().as_deref());
                begin(engine);
            }
            _ => {}
        }
    }
}

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let mut flag = |name: &str| match args.iter().position(|a| a == name) {
        Some(i) => {
            args.remove(i);
            true
        }
        None => false,
    };
    let time = flag("--time");
    let collect_steps = flag("--collect-steps");
    let stop_moved = flag("--stop-moved");
    let trace = flag("--trace");
    let playback = !flag("--no-playback");
    let offline = flag("--offline");
    let fresh = flag("--fresh");
    let n8 = flag("--n8");
    let chain = flag("--chain");
    // N8's replay by partition (MS6 step 7c): none, or of every translation
    // it can be, where it would not pay too (the harness).
    if flag("--no-replay") {
        lily_engine_music::set_replay(false);
    }
    if flag("--replay-all") {
        lily_engine_music::set_replay(true);
        lily_engine_music::set_replay_partitions(1);
    }
    let cells = !flag("--no-cells");
    let preview = flag("--preview");
    let perform = flag("--perform");
    if flag("--check-cells") {
        lily_engine_layout::cell::set_check(true);
    }
    let mut breaks = Vec::new();
    let pdf = option(&mut args, "--pdf").map(PathBuf::from);
    let visible =
        option(&mut args, "--visible").map(|v| v.parse::<u32>().unwrap_or_else(|_| usage()));
    while let Some(when) = option(&mut args, "--break") {
        breaks.push(when);
    }
    let mut packs = Vec::new();
    while let Some(pack) = option(&mut args, "--pack") {
        packs.push(PathBuf::from(pack));
    }
    // A step's budget, with which compiles return between steps (H3); and
    // how many steps a compile runs before a newer one stops it.
    let number = |text: Option<String>| text.map(|t| t.parse::<f64>().unwrap_or_else(|_| usage()));
    let step_ms = number(option(&mut args, "--step-ms"));
    let stop_at = number(option(&mut args, "--stop-at")).map(|n| n as usize);
    if let Some(n) = number(option(&mut args, "--collect-every")) {
        lily_engine_scheme::set_collect_every(n as u32);
    }
    let mut edited = Vec::new();
    while let Some(file) = option(&mut args, "--edited") {
        edited.push(file);
    }
    let fonts = option(&mut args, "--fonts").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts"),
        PathBuf::from,
    );
    let out = option(&mut args, "--out").unwrap_or_else(|| usage());
    let lily = option(&mut args, "--lily")
        .map(PathBuf::from)
        .or_else(support::lily_scm)
        .unwrap_or_else(|| fail("no scm/lily: install LilyPond 2.26.0 or pass --lily"));
    if args.is_empty() || !(edited.is_empty() || edited.len() == args.len()) {
        usage();
    }
    let mut vm = support::library_with(&lily, lily_engine_core::install)
        .unwrap_or_else(|e| fail(format!("{}: {e}", lily.display())));
    let scheme = vm.write_image().unwrap_or_else(|e| fail(format!("{e:?}")));
    let image = write_image(&scheme);
    let packs: Vec<(String, Vec<u8>)> = packs
        .iter()
        .map(|pack| {
            let data = fs::read(pack).unwrap_or_else(|e| fail(format!("{}: {e}", pack.display())));
            let name = pack.file_stem().unwrap_or_default().to_string_lossy();
            (name.into_owned(), data)
        })
        .collect();
    let new_engine = || {
        let mut engine = Engine::start(&image).unwrap_or_else(|e| fail(e));
        engine.set_trace(trace);
        engine.set_playback(playback);
        engine.set_breaks(&breaks);
        engine.set_check_n8(n8);
        engine.set_cells(cells);
        engine.set_preview(preview);
        engine.set_pdf(pdf.is_some());
        engine.set_performs(!perform);
        engine.set_returns(step_ms.is_some());
        engine.set_collect_every_step(collect_steps);
        if let Some(page) = visible {
            engine.set_visible_page(page);
        }
        for (name, data) in &packs {
            engine.put(MODULE, name, Some(data));
        }
        engine
    };
    let host = Host {
        datadir: lily.join("../.."),
        lily,
        fonts,
        offline,
        budget: step_ms.unwrap_or(10.0),
        steps: Default::default(),
    };
    let mut engine = new_engine();
    let mut failed = false;
    let mut revision = 0;
    // The last text before an edit's outputs, for a `--preview`'s `.before`.
    let mut before: Vec<(String, Vec<u8>)> = Vec::new();
    for (n, file) in args.iter().enumerate() {
        if fresh && n > 0 {
            engine = new_engine();
        }
        let read = |file: &str| fs::read(file).unwrap_or_else(|e| fail(format!("{file}: {e}")));
        let mut text = read(file);
        let mut was = Vec::new();
        let out = match edited.get(n) {
            Some(after) => {
                // The text before the edit, compiled first in the same
                // engine, and its outputs dropped: the incremental compile.
                let goes_on = chain && n > 0 && args[n - 1] == *file;
                if !goes_on {
                    before.clear();
                }
                if !fresh && !goes_on {
                    engine.put(FILE, file, Some(&text));
                    revision += 1;
                    run(&mut engine, revision, file, &host);
                    if preview {
                        before.extend_from_slice(engine.outputs());
                    }
                }
                was = core::mem::replace(&mut text, read(after));
                Path::new(&out).join((n + 1).to_string())
            }
            None => PathBuf::from(&out),
        };
        fs::create_dir_all(&out).unwrap_or_else(|e| fail(format!("{}: {e}", out.display())));
        engine.put(FILE, file, Some(&text));
        revision += 1;
        // A compile a newer one stops (H3, B8), the file's place in the list
        // moving where it stops; a stop for the host ends it sooner.
        if let Some(steps) = stop_at {
            if stop_moved {
                // On the line of the edit, as typing is: one place in the text.
                let same = text.iter().zip(&was).take_while(|(a, b)| a == b).count();
                let at = text[..same]
                    .iter()
                    .rposition(|&b| b == b'\n')
                    .map_or(0, |at| at + 1);
                let stopped = [&text[..at], b"% stopped\n", &text[at..]].concat();
                engine.put(FILE, file, Some(&stopped));
            }
            engine.compile(revision, file);
            for _ in 0..steps * (n + 1) {
                if engine.step(host.budget) != MORE {
                    break;
                }
            }
            revision += 1;
            engine.put(FILE, file, Some(&text));
        }
        let start = Instant::now();
        let (asked, stops) = run(&mut engine, revision, file, &host);
        if time {
            let t = engine.times();
            let (reused, parsed) = engine.reused();
            println!(
                "{file}\t{:.3}\t{asked}\t{stops}\t{:.3}\t{:.3}\t{}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{}\t{:.3}\t{:.3}\t{reused}\t{parsed}\t{:.3}\t{}\t{}\t{:.3}\t{}\t{}\t{:.3}\t{:.3}",
                start.elapsed().as_secs_f64() * 1000.0,
                t.parse_ms,
                t.translate_ms,
                t.translations,
                t.longest_ms,
                t.lay_out_ms,
                t.stencils_ms,
                t.write_ms,
                t.live_grobs,
                t.perform_ms,
                t.midi_ms,
                t.longest_step_ms,
                t.longest_step,
                t.steps,
                t.visible_ms,
                t.longest_from,
                t.collections,
                t.collect_ms,
                t.longest_wall_ms
            );
        }
        let stem = Path::new(file)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        if time && step_ms.is_some() {
            let lines: Vec<String> = host
                .steps
                .take()
                .iter()
                .map(|(ms, stage)| format!("{ms:.3}\t{stage}\n"))
                .collect();
            let path = out.join(format!("{stem}.steps.tsv"));
            fs::write(path, lines.concat()).unwrap_or_else(|e| fail(e));
        }
        failed |= engine.ended();
        if n8 && edited.get(n).is_some() && !fresh {
            let path = out.join(format!("{stem}.n8.tsv"));
            fs::write(&path, n8_lines(&engine))
                .unwrap_or_else(|e| fail(format!("{}: {e}", path.display())));
        }
        if edited.get(n).is_some() && !fresh {
            let mut lines = String::new();
            for r in engine.cell_reports() {
                lines.push_str(&format!(
                    "cells\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    r.cells,
                    r.reused,
                    r.grobs,
                    r.grobs_reused,
                    r.kept,
                    r.no_key,
                    r.no_range,
                    r.impure,
                    r.reads_y,
                    r.coupled,
                    r.lines,
                    r.lines_reused,
                    r.lines_wrong,
                    r.before,
                    r.before_reused,
                    r.before_grobs,
                    r.before_grobs_reused,
                    r.before_kept,
                    r.skylines,
                    r.skylines_reused,
                    r.skylines_moved,
                    r.before_moved,
                    r.before_places
                ));
                for u in &r.uncovered {
                    lines.push_str(&format!("uncovered\t{u}\n"));
                }
            }
            let path = out.join(format!("{stem}.cells.tsv"));
            fs::write(&path, lines).unwrap_or_else(|e| fail(format!("{}: {e}", path.display())));
        }
        if preview && edited.get(n).is_some() && !fresh {
            let mut lines = String::new();
            for r in engine.page_reports() {
                lines.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    r.name,
                    r.cells,
                    r.sent,
                    u8::from(r.replaced),
                    r.sent_bytes,
                    r.page_bytes,
                    r.entries,
                    r.taken,
                    engine.markup_bytes()
                ));
            }
            let path = out.join(format!("{stem}.preview.tsv"));
            fs::write(&path, lines).unwrap_or_else(|e| fail(format!("{}: {e}", path.display())));
            for (name, _) in engine.outputs() {
                let Some(page) = name.strip_suffix(".cells") else {
                    continue;
                };
                let svg = format!("{page}.svg");
                let shown = before.iter().find(|(n, _)| *n == svg);
                let path = out.join(format!("{page}.before"));
                fs::write(&path, shown.map_or(&[][..], |(_, data)| data))
                    .unwrap_or_else(|e| fail(format!("{}: {e}", path.display())));
            }
        }
        if chain && preview {
            before = engine.outputs().to_vec();
        }
        for (name, data) in engine.outputs() {
            let file = match name.as_str() {
                "stderr" => {
                    failed |= [&b"fatal error:"[..], GUILE_ERROR]
                        .iter()
                        .any(|m| data.windows(m.len()).any(|w| w == *m));
                    format!("{stem}.err")
                }
                "diagnostics.json" => format!("{stem}.json"),
                "trace.tsv" | "trace.log" | "grobs.tsv" | "layout.tsv" | "columns.tsv"
                | "line.tsv" | "c5.tsv" | "skylines.tsv" => {
                    format!("{stem}.{name}")
                }
                // The pages, and those the score's Scheme writes,
                // `timing.ly`'s among them.
                name if !name.contains('/') => name.into(),
                _ => continue,
            };
            let path = out.join(file);
            fs::write(&path, data).unwrap_or_else(|e| fail(format!("{}: {e}", path.display())));
            if let (Some(dir), Some(book)) = (&pdf, name.strip_suffix(".list")) {
                write_pdf(dir, &out.join(book), data);
            }
        }
        if perform {
            // The performance after the page: its MIDI files and map.
            run_as(&mut engine, revision, file, &host, true);
            for (name, data) in engine.outputs() {
                let file = match name.as_str() {
                    "stderr" => format!("{stem}.perform.err"),
                    "diagnostics.json" => format!("{stem}.perform.json"),
                    name if !name.contains('/') => name.into(),
                    _ => continue,
                };
                let path = out.join(file);
                fs::write(&path, data).unwrap_or_else(|e| fail(format!("{}: {e}", path.display())));
            }
        }
    }
    process::exit(i32::from(failed));
}

/// `--n8`'s file: a line per translation of the last compile, its
/// [`lily_engine_music::Change`], then what its whole runs contradicted.
fn n8_lines(engine: &Engine) -> String {
    let moment = |m: Option<lily_engine_music::Moment>| match m {
        None => "-".to_string(),
        Some(m) => format!(
            "{}/{}{:+}/{}",
            m.main.numerator(),
            m.main.denominator(),
            m.grace.numerator(),
            m.grace.denominator()
        ),
    };
    let index = |i: Option<usize>| i.map_or("-".to_string(), |i| i.to_string());
    let mut text = String::new();
    for (k, c) in engine.changes().iter().enumerate() {
        let columns: Vec<String> = c.columns.iter().map(|&m| moment(Some(m))).collect();
        // With partitions (MS6 step 6): the timesteps each runs, and the
        // translators times timesteps run, without partitions, and whole;
        // and were a partition's writes outside itself replayed (step 7a).
        let parts = c.parts.as_ref().map_or("-".to_string(), |p| {
            let runs: Vec<String> = p.runs.iter().map(u32::to_string).collect();
            format!(
                "{} {} {} {} {}",
                runs.join(","),
                p.work,
                p.unpartitioned,
                p.whole,
                p.reads_only
            )
        });
        // The snapshot it was resumed from (MS6 step 5's rest, D241).
        let resumed = engine.resumed_at().get(k).copied().flatten();
        // Whether it then converged and ended as the run before did (D243).
        let spliced = if engine.spliced().get(k).copied().unwrap_or(false) {
            "spliced"
        } else {
            "-"
        };
        // What it replayed (MS6 step 7c): the attempts, the partitions'
        // translator calls made and those made again from the log.
        let r = engine.replayed().get(k).copied().unwrap_or_default();
        let replayed = format!("{} {} {}", r.attempts, r.run, r.replayed);
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            k + 1,
            moment(c.changed),
            index(c.resume),
            index(c.converged),
            c.steps,
            c.run,
            columns.join(" "),
            parts,
            index(resumed),
            spliced,
            replayed
        ));
    }
    // Each translation's partitions, the shared level's translators and
    // theirs, and why all are one if they are.
    for (k, kept) in engine.kept().iter().enumerate() {
        let p = &kept.parts;
        let weights: Vec<String> = p.weights.iter().map(u32::to_string).collect();
        text.push_str(&format!(
            "parts\t{}\t{}\t{}\t{}\n",
            k + 1,
            weights.join(","),
            p.merged_all.as_deref().unwrap_or("-"),
            p.keys.join("\t")
        ));
    }
    for wrong in engine.n8_wrong() {
        text.push_str(&format!("wrong\t{wrong}\n"));
    }
    text
}

/// A part's module of `files` from `lily` ([`write_part`]); none if one is
/// not there.
fn part(lily: &Path, files: &[&str]) -> Option<Vec<u8>> {
    let texts = files
        .iter()
        .map(|file| fs::read(lily.join(file)).ok())
        .collect::<Option<Vec<_>>>()?;
    let named: Vec<(&str, &[u8])> = files
        .iter()
        .copied()
        .zip(texts.iter().map(Vec::as_slice))
        .collect();
    Some(write_part(&named))
}

fn usage() -> ! {
    eprintln!(
        "usage: compile [--lily <scm/lily>] [--time] [--trace] [--no-playback] [--pack <file.pack>]… [--fonts <dir> | --offline] [--break <when>]… [--fresh] [--edited <file.ly>]… [--chain] [--n8] [--no-replay | --replay-all] [--no-cells] [--check-cells] [--preview] [--perform] [--pdf <dir>] --out <dir> <file.ly>…"
    );
    process::exit(2)
}

/// The PDF of the display list `list` written to `path`, its text fonts
/// read from `dir`, and the writer's warnings beside it (`--pdf`).
fn write_pdf(dir: &Path, path: &Path, list: &[u8]) {
    let doc = lily_engine_pdf::Document::decode(list)
        .unwrap_or_else(|_| fail(format!("{}: not a display list", path.display())));
    let otfs: Vec<(String, Vec<u8>)> = doc
        .fonts
        .iter()
        .filter_map(|name| {
            // LilyPond's fonts/otf names TeX Gyre's in lower case (E23).
            let otf = fs::read(dir.join(format!("{name}.otf")))
                .or_else(|_| fs::read(dir.join(format!("{}.otf", name.to_lowercase()))))
                .ok()?;
            Some((name.clone(), otf))
        })
        .collect();
    let fonts: Vec<lily_engine_pdf::Font<'_>> = otfs
        .iter()
        .map(|(name, otf)| lily_engine_pdf::Font { name, otf })
        .collect();
    let written = lily_engine_pdf::write(&doc, &fonts);
    fs::write(path, &written.pdf).unwrap_or_else(|e| fail(format!("{}: {e}", path.display())));
    let log = path.with_extension("pdf.log");
    let text: String = written.warnings.iter().map(|w| format!("{w}\n")).collect();
    fs::write(&log, text).unwrap_or_else(|e| fail(format!("{}: {e}", log.display())));
}
