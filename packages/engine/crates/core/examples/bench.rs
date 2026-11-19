//! `cargo run --release -p lily-engine-core --example bench -- [start] <file.scm> …`:
//! each file evaluated by a fresh [`Bench`], as `lsb_init` or `lsb_restore`
//! and `lsb_eval` do in wasm. Prints one JSON object per file, `{"file",
//! "ok", "output", "startMs", "ms"}` and the stats, which
//! `tools/bench/test/wasm.test.mjs` and `image.test.mjs` compare with the
//! wasm's. The start is one of:
//!
//! - nothing: [`Bench::new`];
//! - `--library <scm/lily>`: `lily.scm`'s core files loaded from source
//!   into `(lily)`, with what the parser needs besides and the stubs of the
//!   parts on demand, and `ly/declarations-init.ly` with the files it
//!   includes parsed and run after them, as the parser's runner loads them
//!   (`crates/parser/tests/support`, D78); a part loads from there when one
//!   of its stubs is called (D74);
//! - `--image <file>`: [`Bench::restore`] of that image.
//!
//! `--write-image <scm/lily> <file> [setup.scm]` writes the image of a VM
//! that loaded them so, which `scripts/build.mjs` ships as `scheme.image`,
//! and prints `{"bytes", "loadMs", "writeMs"}`. With `setup.scm`, that file
//! runs before the image is written, as a probe's music is made for
//! `image.test.mjs` (`probe-music.scm`).
//!
//! `--write-core-image <scheme.image> <file>` writes the core's image of it
//! (`lily_engine_core::write_image`), which `scripts/build.mjs` ships as
//! `core.image` beside `scheme.image`, and `lily_start` restores.
//!
//! `--write-part <scm/lily> <part> <file>` writes the module of one of
//! S6's parts the host gives as modules (`lily_engine_core::PART_MODULES`,
//! I2), its files read from `scm/lily`, which `scripts/build.mjs` ships as
//! `<part>.part`; the part's files must be those the image's `%parts`
//! gives it (`support`'s `PARTS`), then its modules' own (`MODULE_FILES`).
//!
//! `--rewrite-image <image> <file>` restores the image and writes it again
//! (it collects first), so that two builds' restores of the same image can
//! be compared byte for byte (MS2 step 31).
//!
//! `--svg <image> <packs> <runs> [--root <dir>]… [--out <dir>] <pages.scm>…`
//! restores the image, gives it every `emmentaler-*.pack` in `packs`, and
//! writes each `(page …)` record of the files `runs` times with
//! [`Bench::svg`], as `lsb_svg` does in wasm (MS3 step 9): one JSON line a
//! page, `{"file", "number", "bytes", "ms"}`, `ms` each write's
//! milliseconds in order; with `--out`, the page written under its file
//! name there.
use lily_engine_core::bench::{Bench, STATS};
use lily_engine_core::now;
use std::path::Path;

// The image writer needs only the loading and the init files.
#[allow(dead_code)]
#[path = "../../parser/tests/support/mod.rs"]
mod support;

/// A bench whose `(lily)` loaded the library files from `lily`, LilyPond's
/// `scm/lily/`, with what the parser needs of LilyPond's files besides, and
/// the whole init after them (`support::load_library`).
fn library(lily: &Path) -> Bench {
    let mut bench = Bench::new();
    support::load_library(bench.vm(), lily).unwrap_or_else(|e| panic!("{e}"));
    bench
}

/// A bench of `library` that loads a part's files from `lily` when one of
/// its stubs is called (D74), as the parser's runner does.
fn library_with_parts(lily: &Path) -> Bench {
    let mut bench = library(lily);
    bench.vm().set_part_source(Some(support::part_source(lily)));
    bench
}

enum Start {
    New,
    Library(String),
    Image(Vec<u8>),
}

fn read(file: &str) -> Vec<u8> {
    std::fs::read(file).unwrap_or_else(|e| panic!("{file}: {e}"))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--write-core-image") {
        let [_, scheme, out] = &args[..] else {
            panic!("--write-core-image <scheme.image> <file>")
        };
        let image = lily_engine_core::write_image(&read(scheme));
        std::fs::write(out, &image).unwrap_or_else(|e| panic!("{out}: {e}"));
        println!("{{\"bytes\":{}}}", image.len());
        return;
    }
    if args.first().map(String::as_str) == Some("--write-part") {
        let [_, lily, part, out] = &args[..] else {
            panic!("--write-part <scm/lily> <part> <file>")
        };
        let Some((_, files)) = lily_engine_core::PART_MODULES
            .iter()
            .find(|(p, _)| p == part)
        else {
            panic!("{part} is no part the host gives as a module")
        };
        let image = support::PARTS
            .iter()
            .find(|(p, _)| p == part)
            .map(|(_, f)| *f);
        // The image's, then the modules of their own the part's data holds.
        let own: Vec<&str> = support::MODULE_FILES
            .iter()
            .filter(|(p, _, _)| p == part)
            .map(|&(_, _, file)| file)
            .collect();
        let expected: Option<Vec<&str>> = image.map(|f| f.iter().copied().chain(own).collect());
        assert_eq!(
            expected.as_deref(),
            Some(*files),
            "{part}'s files are not the image's"
        );
        let texts: Vec<Vec<u8>> = files
            .iter()
            .map(|file| read(&Path::new(lily).join(file).to_string_lossy()))
            .collect();
        let named: Vec<(&str, &[u8])> = files
            .iter()
            .copied()
            .zip(texts.iter().map(Vec::as_slice))
            .collect();
        std::fs::write(out, lily_engine_core::write_part(&named)).expect("the part writes");
        return;
    }
    if args.first().map(String::as_str) == Some("--rewrite-image") {
        let [_, image, out] = &args[..] else {
            panic!("--rewrite-image <image> <file>")
        };
        let mut bench = Bench::restore(&read(image)).unwrap_or_else(|e| panic!("{e:?}"));
        let image = bench.write_image().unwrap_or_else(|e| panic!("{e:?}"));
        std::fs::write(out, &image).unwrap_or_else(|e| panic!("{out}: {e}"));
        println!("{{\"bytes\":{}}}", image.len());
        return;
    }
    if args.first().map(String::as_str) == Some("--write-image") {
        let (lily, out, setup) = match &args[..] {
            [_, lily, out] => (lily, out, None),
            [_, lily, out, setup] => (lily, out, Some(setup)),
            _ => panic!("--write-image <scm/lily> <file> [setup.scm]"),
        };
        let started = now();
        let mut bench = library(Path::new(lily));
        if let Some(setup) = setup {
            let text = std::fs::read_to_string(setup).unwrap_or_else(|e| panic!("{setup}: {e}"));
            assert!(bench.eval(&text), "{setup}: {}", bench.output());
        }
        let loaded = now();
        let image = bench.write_image().unwrap_or_else(|e| panic!("{e:?}"));
        let written = now();
        std::fs::write(out, &image).unwrap_or_else(|e| panic!("{out}: {e}"));
        let mut line = format!("{{\"bytes\":{},\"loadMs\":", image.len());
        push_num(&mut line, loaded - started);
        line.push_str(",\"writeMs\":");
        push_num(&mut line, written - loaded);
        println!("{line}}}");
        return;
    }
    if args.first().map(String::as_str) == Some("--svg") {
        return svg(&args[1..]);
    }
    let start = match args.first().map(String::as_str) {
        Some("--library") => Start::Library(args[1].clone()),
        Some("--image") => Start::Image(read(&args[1])),
        _ => Start::New,
    };
    let files = if matches!(start, Start::New) {
        &args[..]
    } else {
        &args[2..]
    };
    for file in files {
        let text = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{file}: {e}"));
        let started = now();
        let mut bench = match &start {
            Start::New => Bench::new(),
            Start::Library(lily) => library_with_parts(Path::new(lily)),
            Start::Image(image) => Bench::restore(image).unwrap_or_else(|e| panic!("{e:?}")),
        };
        let start_ms = now() - started;
        let started = now();
        let ok = bench.eval(&text);
        let ms = now() - started;
        let mut line = String::from("{\"file\":");
        push_json(&mut line, file);
        line.push_str(if ok { ",\"ok\":true" } else { ",\"ok\":false" });
        line.push_str(",\"output\":");
        push_json(&mut line, bench.output());
        line.push_str(",\"startMs\":");
        push_num(&mut line, start_ms);
        line.push_str(",\"ms\":");
        push_num(&mut line, ms);
        for (name, value) in STATS.iter().zip(bench.stats()) {
            line.push_str(",\"");
            line.push_str(name);
            line.push_str("\":");
            push_num(&mut line, *value);
        }
        line.push('}');
        println!("{line}");
    }
}

/// `--svg`'s pages, each written `runs` times.
fn svg(args: &[String]) {
    let [image, packs, runs, rest @ ..] = args else {
        panic!("--svg <image> <packs> <runs> [--root <dir>]… [--out <dir>] <pages.scm>…")
    };
    let runs: u32 = runs.parse().expect("<runs> is a number");
    let (mut roots, mut out, mut files) = (Vec::new(), None, Vec::new());
    let mut rest = rest.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--root" => roots.push(rest.next().expect("--root <dir>").as_str()),
            "--out" => out = Some(Path::new(rest.next().expect("--out <dir>"))),
            _ => files.push(arg),
        }
    }
    let mut bench = Bench::restore(&read(image)).unwrap_or_else(|e| panic!("{e:?}"));
    let mut names: Vec<_> = std::fs::read_dir(packs)
        .unwrap_or_else(|e| panic!("{packs}: {e}"))
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.starts_with("emmentaler-") && name.ends_with(".pack")
        })
        .collect();
    // The brace's parts in their order.
    names.sort();
    for path in names {
        assert!(
            bench.pack(&read(path.to_str().unwrap())),
            "{}: {}",
            path.display(),
            bench.output()
        );
    }
    if let Some(out) = out {
        std::fs::create_dir_all(out).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
    }
    for file in files {
        let text = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{file}: {e}"));
        for record in text.lines().filter(|line| line.starts_with("(page ")) {
            let ms: Vec<f64> = (0..runs)
                .map(|_| {
                    bench
                        .svg(record, &roots, 1)
                        .unwrap_or_else(|| panic!("{file}: {}", bench.output()))
                })
                .collect();
            // `(page "<file>" <number> …`
            let mut head = record["(page ".len()..].splitn(3, ' ');
            let name = head.next().unwrap().trim_matches('"');
            let number = head.next().unwrap();
            if let Some(out) = out {
                std::fs::write(out.join(name), bench.output())
                    .unwrap_or_else(|e| panic!("{name}: {e}"));
            }
            let mut line = String::from("{\"file\":");
            push_json(&mut line, name);
            line.push_str(&format!(
                ",\"number\":{number},\"bytes\":{},\"ms\":[",
                bench.output().len()
            ));
            for (i, ms) in ms.iter().enumerate() {
                if i > 0 {
                    line.push(',');
                }
                push_num(&mut line, *ms);
            }
            line.push_str("]}");
            println!("{line}");
            // A page's values are garbage once it is written.
            bench.vm().collect();
        }
    }
}

fn push_int(out: &mut String, n: i64) {
    out.push_str(&n.to_string());
}

/// At most three decimals, without trailing zeros, as the spike wrote them.
fn push_num(out: &mut String, v: f64) {
    let milli = (v * 1000.0).round() as i64;
    if milli < 0 {
        out.push('-');
    }
    let m = milli.unsigned_abs();
    push_int(out, (m / 1000) as i64);
    let mut frac = m % 1000;
    if frac != 0 {
        out.push('.');
        let mut div = 100;
        while frac != 0 {
            out.push((b'0' + (frac / div) as u8) as char);
            frac %= div;
            div /= 10;
        }
    }
}

/// A JSON string literal.
fn push_json(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}
