//! The engine's Scheme (`lily_engine_scheme`) behind chapter 03 Appendix B's
//! bench ABI, so that `tools/bench/run.mjs` and the harness's `scheme.html`
//! time it in wasm, and `examples/bench.rs` runs the same code natively; its
//! `(lily)` has the music crate's values (`lily_engine_music`) and layout's
//! (`lily_engine_layout`). A bench may
//! also start from an image of such a VM (chapter 03 §5.6), which
//! `examples/bench.rs` writes natively and `lsb_restore` restores in wasm.
//! It also writes a page of the references in the file form and times it
//! ([`Bench::svg`], MS3 step 9), for README §4.3's SVG phase.

use lily_engine_layout::{Stencil, font};
use lily_engine_scheme::{ImageError, Vm};
use lily_engine_svg::{Paper, write_page};

/// What [`Bench::stats`] holds, in order.
pub const STATS: [&str; 6] = [
    "applications",
    "collections",
    "longestPauseMs",
    "pauseMs",
    "liveBytes",
    "memory",
];

pub struct Bench {
    vm: Vm,
    output: String,
    stats: [f64; STATS.len()],
}

impl Default for Bench {
    fn default() -> Bench {
        Bench::new()
    }
}

impl Bench {
    /// A VM with its primitives and preludes and the `ly:` functions of the
    /// music model and layout ([`crate::install`]), its collections timed by
    /// the host's clock.
    pub fn new() -> Bench {
        let mut vm = Vm::new();
        let installed = crate::install(&mut vm);
        debug_assert!(
            installed.is_ok(),
            "the music and layout crates' preludes run"
        );
        Bench::with(vm)
    }

    /// The VM that [`Bench::write_image`] wrote, restored.
    pub fn restore(image: &[u8]) -> Result<Bench, ImageError> {
        Bench::restore_marked(image, &mut |_| {})
    }

    /// [`Bench::restore`], calling `mark` with each of
    /// [`lily_engine_scheme::RESTORE_MARKS`] as that part ends.
    pub fn restore_marked(
        image: &[u8],
        mark: &mut dyn FnMut(&'static str),
    ) -> Result<Bench, ImageError> {
        Ok(Bench::with(Vm::restore_marked(
            image,
            crate::EXTENSIONS,
            mark,
        )?))
    }

    fn with(mut vm: Vm) -> Bench {
        vm.heap_mut().set_clock(crate::now);
        Bench {
            vm,
            output: String::new(),
            stats: [0.0; STATS.len()],
        }
    }

    /// The VM as an image, which [`Bench::restore`] restores; it collects.
    pub fn write_image(&mut self) -> Result<Vec<u8>, ImageError> {
        self.vm.write_image()
    }

    pub fn vm(&mut self) -> &mut Vm {
        &mut self.vm
    }

    /// Reads and evaluates `text` as Guile's `primitive-load` does. Its output
    /// is what it printed, then, if it failed, the error as Guile prints it.
    pub fn eval(&mut self, text: &str) -> bool {
        let (applications, collections, pauses) = (
            self.vm.applications(),
            self.vm.heap().stats().collections,
            self.vm.heap().stats().pause_ms,
        );
        let result = self.vm.eval_text(text, None);
        self.output = self.vm.take_output();
        if let Err(e) = &result {
            if !self.output.is_empty() && !self.output.ends_with('\n') {
                self.output.push('\n');
            }
            self.output.push_str(&e.text(self.vm.heap()));
            self.output.push('\n');
        }
        let heap = self.vm.heap().stats();
        self.stats = [
            (self.vm.applications() - applications) as f64,
            (heap.collections - collections) as f64,
            heap.longest_pause_ms,
            heap.pause_ms - pauses,
            heap.live_bytes as f64,
            crate::memory() as f64,
        ];
        result.is_ok()
    }

    /// Gives layout's fonts W1's pack `pack` ([`font::put_pack`]); its
    /// output is the error if it was refused.
    pub fn pack(&mut self, pack: &[u8]) -> bool {
        self.output.clear();
        let put = font::put_pack(&mut self.vm, pack);
        if let Err(e) = &put {
            self.output = e.text(self.vm.heap());
        }
        put.is_ok()
    }

    /// Writes the page of `record`, a line `(page <file> <number> <X> <Y>
    /// <expression>)` of a `pages.scm` that step 6 captured, in the file
    /// form, `reps` times, and gives the milliseconds the writes took in
    /// all, by the host's clock (MS3 step 9). The record is read into
    /// layout's stencil (`%dump->stencil`) before the clock starts; the
    /// page is written as `crates/svg`'s write example writes it, at staff
    /// size 20, a link's file named below the first of `roots` it is in.
    /// Its output is the last write's SVG, or the error.
    pub fn svg(&mut self, record: &str, roots: &[&str], reps: u32) -> Option<f64> {
        self.output.clear();
        let read = format!("(apply (@@ (lily) %dump->stencil) (list-tail '{record} 3))");
        let stencil = match self.vm.eval_text(&read, None) {
            Ok(value) => match self.vm.heap().foreign::<Stencil>(value) {
                Some(stencil) => *stencil,
                None => {
                    self.output = "the record is no page's stencil".into();
                    return None;
                }
            },
            Err(e) => {
                self.output = e.text(self.vm.heap());
                return None;
            }
        };
        let paper = Paper {
            output_scale: Paper::output_scale_of(20.0),
            tree_roots: roots,
        };
        let started = crate::now();
        for _ in 0..reps.max(1) {
            match write_page(&mut self.vm, &stencil, &paper) {
                Ok(page) => self.output = page.svg,
                Err(e) => {
                    self.output = e.text(self.vm.heap());
                    return None;
                }
            }
        }
        Some(crate::now() - started)
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    /// The last [`Bench::eval`]'s counts, named by [`STATS`]: its
    /// applications, collections and their milliseconds, the longest pause
    /// since the VM was made, the bytes live after the last collection, and
    /// linear memory (0 natively).
    pub fn stats(&self) -> &[f64; STATS.len()] {
        &self.stats
    }
}
