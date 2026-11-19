//! A score over the memory's cap is a diagnostic, and the engine goes on
//! (D256, D257, D259). A file of its own: its global allocator counts every
//! byte the process holds, and what stands for wasm's cap is the process's
//! ([`Engine::set_room`]).

use lily_engine_core::{DONE, Engine, FILE, MODULE, MORE, NEEDS, write_image};
use lily_engine_parser::source::DATADIR;
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

#[allow(dead_code)]
#[path = "../../parser/tests/support/mod.rs"]
mod support;

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);
/// The bytes past which the engine is short of memory.
static CAP: AtomicUsize = AtomicUsize::new(usize::MAX);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, at: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(at, layout) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

fn short(extra: usize) -> bool {
    LIVE.load(Ordering::Relaxed) + extra > CAP.load(Ordering::Relaxed)
}

/// A compile of `text` run to its end in steps; its outputs, and how many
/// steps it returned.
fn compile(engine: &mut Engine, text: &str, datadir: &Path) -> (Vec<(String, Vec<u8>)>, usize) {
    let root = "/s/a.ly";
    engine.put(FILE, root, Some(text.as_bytes()));
    engine.compile(1, root);
    let mut steps = 0;
    loop {
        match engine.step(1e9) {
            MORE => steps += 1,
            DONE => return (engine.outputs().to_vec(), steps),
            NEEDS => {
                let (kind, name) = engine.needs().map(|(k, n)| (k, n.to_string())).unwrap();
                let data = if kind == MODULE {
                    let fonts =
                        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts");
                    std::fs::read(fonts.join(format!("{name}.pack"))).ok()
                } else {
                    let rest = name.strip_prefix(DATADIR).unwrap_or(&name);
                    std::fs::read(datadir.join(rest.trim_start_matches('/'))).ok()
                };
                engine.put(kind, &name, data.as_deref());
                engine.compile(1, root);
            }
            step => panic!("step {step}"),
        }
    }
}

fn text_of(outputs: &[(String, Vec<u8>)], name: &str) -> String {
    let file = outputs.iter().find(|(n, _)| n == name);
    String::from_utf8_lossy(&file.expect("the output").1).into_owned()
}

fn pages(outputs: &[(String, Vec<u8>)]) -> Vec<Vec<u8>> {
    outputs
        .iter()
        .filter(|(n, _)| n.ends_with(".svg"))
        .map(|(_, bytes)| bytes.clone())
        .collect()
}

fn staff(bars: usize) -> String {
    let music = "c'4 d' e' f' | ".repeat(bars);
    format!("\\version \"2.26.0\"\n\\score {{ \\new Staff {{ {music} }} \\layout {{ }} }}\n")
}

#[test]
fn a_score_over_the_cap_is_a_diagnostic_and_the_engine_goes_on() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let datadir = lily.join("../..");
    let mut vm = support::library_with(&lily, lily_engine_core::install).unwrap();
    let image = write_image(&vm.write_image().expect("the VM is at rest"));
    drop(vm);
    let mut engine = Engine::start(&image).expect("the image restores");
    engine.set_returns(true);
    engine.set_room(short);

    let small = staff(4);
    let (first, _) = compile(&mut engine, &small, &datadir);
    assert_eq!(pages(&first).len(), 1, "{}", text_of(&first, "stderr"));

    // Room for the small score and little more.
    CAP.store(LIVE.load(Ordering::Relaxed) + (6 << 20), Ordering::Relaxed);
    let big = staff(300);
    let (over, steps) = compile(&mut engine, &big, &datadir);
    assert!(steps > 0, "it stopped between its steps");
    assert_eq!(pages(&over).len(), 0, "it has no pages");
    let errors = text_of(&over, "stderr");
    assert!(
        errors.contains("needs more memory than the engine may use (256 MiB)"),
        "{errors}"
    );
    let json = text_of(&over, "diagnostics.json");
    assert!(json.contains("\"severity\":\"error\""), "{json}");

    // The engine goes on: what it compiled before, the same, under the cap.
    let (again, _) = compile(&mut engine, &small, &datadir);
    assert_eq!(pages(&again), pages(&first));

    // Stopped where a translation ends, by what its end would take, which only
    // a compile near the cap meets: a step of any budget leaves there.
    CAP.store(
        LIVE.load(Ordering::Relaxed) + (24 << 20) + (2 << 10) * 1800,
        Ordering::Relaxed,
    );
    let (ended, _) = compile(&mut engine, &big, &datadir);
    assert_eq!(pages(&ended).len(), 0);
    let (again, _) = compile(&mut engine, &small, &datadir);
    assert_eq!(pages(&again), pages(&first));

    // And the score itself once the memory allows.
    CAP.store(usize::MAX, Ordering::Relaxed);
    let (whole, _) = compile(&mut engine, &big, &datadir);
    assert!(!pages(&whole).is_empty(), "{}", text_of(&whole, "stderr"));
}
