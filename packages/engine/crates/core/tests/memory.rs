//! What edits in a row keep (OPEN C15, D255). A file of its own: its global
//! allocator counts every byte the process holds, so nothing else may run.

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

fn compile(engine: &mut Engine, root: &str, text: &str, datadir: &Path) {
    engine.put(FILE, root, Some(text.as_bytes()));
    engine.compile(1, root);
    loop {
        match engine.step(1e9) {
            MORE => {}
            DONE => return,
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

/// Edits in one place of a score leave the engine holding what the first
/// few left it holding: the snapshots before the edit do not outlive their
/// compiles, nor do the pictures of every compile since. They did, 0.7 MiB
/// a compile on T02 and 2.5 on T24, until an instance trapped.
#[test]
fn edits_in_one_place_keep_no_more_than_the_first_few() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let datadir = lily.join("../..");
    let score = |note: &str| {
        let bars = "c'4 d' e' f' | ".repeat(20);
        format!(
            "\\version \"2.26.0\"\n\\score {{ \\new Staff {{ {bars} {note} | {bars} }} \\layout {{ }} }}\n"
        )
    };
    let (text, edit) = (score("g'1"), score("a'1"));
    let mut vm = support::library_with(&lily, lily_engine_core::install).unwrap();
    let image = write_image(&vm.write_image().expect("the VM is at rest"));
    drop(vm);
    let mut engine = Engine::start(&image).expect("the image restores");
    let mut held = Vec::new();
    for round in 0..30 {
        compile(&mut engine, "/s/a.ly", &text, &datadir);
        compile(&mut engine, "/s/a.ly", &edit, &datadir);
        if round == 9 || round == 29 {
            engine.collect();
            held.push(LIVE.load(Ordering::Relaxed));
        }
    }
    assert!(
        held[1] < held[0] + held[0] / 10,
        "bytes held after 20 and 60 compiles: {held:?}"
    );
}
