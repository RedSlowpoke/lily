//! Writes the pages of references in the file form (MS3 step 7): each
//! record of a `pages.scm` that `tools/compare/stencils.ly` captured, read
//! into layout's stencils and fonts (`%dump->stencil`) given W1's packs, and
//! written by the engine's SVG writer under the page's file name in `--out`.
//! One JSON line per page: its file, number, bytes and warnings.
//!
//!   cargo +stable run -p lily-engine-svg --example write -- [--packs <dir>]
//!     [--staff-size <points>] [--root <dir>]… --out <dir> <pages.scm>…
//!
//! `--packs` is the directory of the packs (`dist/engine/fonts` by
//! default), each `emmentaler-*.pack` in it given; `--staff-size` sets
//! `output-scale` (20); a link into a `--root` names its file below it, as
//! the engine's tree does.
use std::path::{Path, PathBuf};

use lily_engine_layout::{Stencil, font};
use lily_engine_scheme::Vm;
use lily_engine_svg::{Paper, write_page};

fn main() {
    let mut packs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts");
    let mut staff_size = 20.0;
    let mut roots = Vec::new();
    let mut out = None;
    let mut files = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .unwrap_or_else(|| fail(&format!("{arg} needs a value")))
        };
        match arg.as_str() {
            "--packs" => packs = PathBuf::from(value()),
            "--staff-size" => {
                staff_size = value()
                    .parse()
                    .unwrap_or_else(|_| fail("--staff-size takes a number"))
            }
            "--root" => roots.push(value()),
            "--out" => out = Some(PathBuf::from(value())),
            _ => files.push(PathBuf::from(arg)),
        }
    }
    let out = out.unwrap_or_else(|| fail("--out <dir> is missing"));
    std::fs::create_dir_all(&out).unwrap_or_else(|e| fail(&format!("{}: {e}", out.display())));
    let mut vm = vm(&packs);
    let roots: Vec<&str> = roots.iter().map(String::as_str).collect();
    let paper = Paper {
        output_scale: Paper::output_scale_of(staff_size),
        tree_roots: &roots,
    };
    for file in files {
        let text = std::fs::read_to_string(&file)
            .unwrap_or_else(|e| fail(&format!("{}: {e}", file.display())));
        for line in text.lines().filter(|line| line.starts_with("(page ")) {
            let (name, number, stencil) = read(&mut vm, line);
            let page = write_page(&mut vm, &stencil, &paper)
                .unwrap_or_else(|e| fail(&format!("{name}: {}", e.text(vm.heap()))));
            std::fs::write(out.join(&name), &page.svg)
                .unwrap_or_else(|e| fail(&format!("{name}: {e}")));
            let warnings: Vec<String> = page.warnings.iter().map(|w| json(w)).collect();
            println!(
                "{{\"file\":{},\"number\":{number},\"bytes\":{},\"warnings\":[{}]}}",
                json(&name),
                page.svg.len(),
                warnings.join(",")
            );
            // A page's values are garbage once it is written.
            vm.collect();
        }
    }
}

/// A VM with the music crate's and layout's functions, `(lily)` current,
/// given every Emmentaler pack in `packs`.
fn vm(packs: &Path) -> Vm {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    lily_engine_layout::install(&mut vm).unwrap();
    vm.eval_text("(set-current-module (resolve-module '(lily)))", None)
        .unwrap();
    let mut names: Vec<PathBuf> = std::fs::read_dir(packs)
        .unwrap_or_else(|e| fail(&format!("{}: {e}", packs.display())))
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.starts_with("emmentaler-") && name.ends_with(".pack")
        })
        .collect();
    // The brace's parts in their order.
    names.sort();
    for path in names {
        let bytes = std::fs::read(&path).unwrap();
        font::put_pack(&mut vm, &bytes)
            .unwrap_or_else(|e| fail(&format!("{}: {}", path.display(), e.text(vm.heap()))));
    }
    vm
}

/// A record `(page <file> <number> <X> <Y> <expression>)`: its file, its
/// number and its stencil.
fn read(vm: &mut Vm, line: &str) -> (String, i64, Stencil) {
    let source = format!(
        "(let ((r '{line})) (list (cadr r) (caddr r) (apply %dump->stencil (list-tail r 3))))"
    );
    let value = vm.eval_text(&source, None).unwrap_or_else(|e| {
        fail(&format!(
            "{}: {}",
            &line[..line.len().min(80)],
            e.text(vm.heap())
        ))
    });
    let heap = vm.heap();
    let items: Vec<_> = (0..3)
        .map(|i| {
            let mut v = value;
            for _ in 0..i {
                v = heap.cdr(v).unwrap();
            }
            heap.car(v).unwrap()
        })
        .collect();
    let name = heap
        .str(items[0])
        .map(|s| s.as_str().to_string())
        .unwrap_or_default();
    let number = heap.to_i64(items[1]).unwrap_or(0);
    let stencil = *heap
        .foreign::<Stencil>(items[2])
        .expect("%dump->stencil gives a stencil");
    (name, number, stencil)
}

fn json(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn fail(message: &str) -> ! {
    eprintln!("write: {message}");
    std::process::exit(2)
}
