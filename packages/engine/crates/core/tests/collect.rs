//! OPEN H15 (MS8 step 3, D268): a value Rust holds unrooted across a call
//! into Scheme is swept only if a collection comes in that window, which
//! timing decides. With every `n`th procedure entry a collection
//! (`set_collect_every`), each window that calls into Scheme `n` times or
//! more is swept, and a compile writes what one without does, byte for byte.
//!
//! Its own test binary: the stride is the process's. The sweep of the 30 at
//! a stride, and T02 at every entry, are `$LILY_CORPUS/device-runs/
//! 2026-11-05-ms8-step3/h15/sweep.sh`'s (a compile at stride 1 takes about
//! 1,400 times as long).

use lily_engine_core::{Engine, FILE, MODULE, NEEDS, write_image};
use lily_engine_parser::source::DATADIR;
use lily_engine_scheme::set_collect_every;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[path = "../../parser/tests/support/mod.rs"]
mod support;

fn fonts() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts")
}

/// `text`'s outputs, traced, in an engine of `image`.
fn compile(image: &[u8], datadir: &Path, text: &str) -> Vec<(String, Vec<u8>)> {
    let mut engine = Engine::start(image).expect("the image restores");
    engine.set_trace(true);
    let root = "/s/a.ly";
    engine.put(FILE, root, Some(text.as_bytes()));
    engine.compile(1, root);
    while engine.step(1e9) == NEEDS {
        let (kind, name) = engine.needs().map(|(k, n)| (k, n.to_string())).unwrap();
        let data = if kind == MODULE {
            std::fs::read(fonts().join(format!("{name}.pack"))).ok()
        } else {
            let rest = name.strip_prefix(DATADIR).unwrap_or(&name);
            std::fs::read(datadir.join(rest.trim_start_matches('/'))).ok()
        };
        engine.put(kind, &name, data.as_deref());
        engine.compile(1, root);
    }
    engine.outputs().to_vec()
}

/// A score through the windows D258 closed (a stem tremolo's and a bracket's
/// stencil moved before their root) and beams, slurs, ties, a tuplet and
/// lyrics: compiled with collections forced at strides that put one in
/// different places, each as without.
#[test]
fn collections_forced_at_procedure_entries_change_no_output() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    if !fonts().join("emmentaler-20.pack").exists() {
        eprintln!("skipped: no dist/engine packs (npm run build)");
        return;
    }
    let mut vm = support::library_with(&lily, lily_engine_core::install).unwrap();
    let image = write_image(&vm.write_image().expect("the VM is at rest"));
    let datadir = lily.join("../..");
    let text = "\\version \"2.26.0\"
\\score { <<
  \\new Staff { c''8[ d'' e'' f''] g''4:32 a''( | \\tuplet 3/2 { b''8 c''' d''' } c'''2~ c'''4 | <c'' e''>2.:16 r4 }
  \\addlyrics { one two three four five six se -- ven eight nine }
  \\new StaffGroup << \\new Staff { \\clef bass c4 d e f | g1 | c,1 } >>
>> \\layout { } }
";
    let plain = compile(&image, &datadir, text);
    for every in [97, 211] {
        set_collect_every(every);
        let forced = compile(&image, &datadir, text);
        set_collect_every(0);
        let differs: Vec<&str> = forced
            .iter()
            .filter(|o| !plain.contains(o))
            .map(|o| o.0.as_str())
            .collect();
        assert!(
            differs.is_empty() && forced.len() == plain.len(),
            "every {every}th entry collecting: {differs:?}"
        );
    }
}
